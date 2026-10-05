use std::io::{ErrorKind, Read};

use async_trait::async_trait;
///
/// Decoder for the AEDAT4 file format decribed [here](https://docs.inivation.com/software/software-advanced-usage/file-formats/aedat-4.0.html).
///
use tokio::io::{self, AsyncReadExt, BufReader};

use xml::{
    attribute::OwnedAttribute,
    reader::{EventReader, XmlEvent},
};

use std::io::Cursor;

use lz4;

use crate::{
    aedat4_data_generated,
    aedat4_header_generated::{self, CompressionType},
    codec::{
        ByteSource, Codec, Decoder, DecoderFactory, ModuleInfo, OutputInfo, Packet, PacketContent,
        VideoMetadata,
    },
    error::{DecodeError, FileReadError},
    file::{AEDAT4_FORMAT, FileFormat},
};

/// Struct representing the state of AEDAT4 codec.
pub struct Aedat4;

async fn read_exact_and_handle_err(
    buffered_reader: &mut BufReader<ByteSource>,
    buffer: &mut [u8],
) -> Result<(), FileReadError> {
    return match buffered_reader
        .read_exact(buffer)
        .await
        .map_err(|err| match err.kind() {
            io::ErrorKind::UnexpectedEof => FileReadError::UnexpectedEOF(
                "unexpected EOF: file ended before magic bytes ended".to_string(),
            ),
            _ => FileReadError::UnexpectedError(
                io::ErrorKind::Other,
                format!("unexpected I/O error: {}", err),
            ),
        }) {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    };
}

fn is_root_output_node(attrs: Vec<OwnedAttribute>) -> bool {
    attrs.iter().any(|a| {
        a.name.local_name == "path"
            && a.value.starts_with("/mainloop/Recorder/outInfo")
            && !a.value.ends_with("info/")
    })
}

impl Codec for Aedat4 {
    fn format(&self) -> &FileFormat {
        &AEDAT4_FORMAT
    }

    fn validate(&self, data: &[u8]) -> Result<(), FileReadError> {
        if !data.starts_with(self.format().magic_bytes.unwrap()) {
            return Err(FileReadError::InvalidMagicBytes);
        }
        Ok(())
    }
}

impl DecoderFactory for Aedat4 {
    async fn open(&self, source: ByteSource) -> Result<Box<dyn Decoder>, FileReadError> {
        // check the magic bytes and parse the IOHeader to get metadata

        let mut buffered_reader = BufReader::new(source);
        let magic_bytes = self.format().magic_bytes.unwrap();

        let mut buffer = vec![0; magic_bytes.len()];

        read_exact_and_handle_err(&mut buffered_reader, &mut buffer).await?;

        self.validate(&buffer)?;

        // read IOHE

        let mut io_buffer = vec![0u8; 4];
        read_exact_and_handle_err(&mut buffered_reader, &mut io_buffer).await?;
        let header_size = u32::from_le_bytes(io_buffer.try_into().unwrap()) as usize;

        let mut iohe_content = vec![0u8; 4 + header_size]; // parser expects a 4 byte size prefix when using `size_prefixed_root_...`
        read_exact_and_handle_err(&mut buffered_reader, &mut iohe_content[4..]).await?;
        let iohe_root = aedat4_header_generated::size_prefixed_root_as_ioheader(&iohe_content)
            .map_err(|_err| {
                FileReadError::UnexpectedError(
                    ErrorKind::InvalidData,
                    String::from(
                        "IOHeader is malformed, expected a valid, size-prefixed flatbuffer.",
                    ),
                )
            })?;

        // extract metadata from xml in iohe_root
        // the xml contains a "outinfo" section, with numbered modules, from 0...n

        let mut module_vector: Vec<ModuleInfo> = vec![];
        let mut current_module = ModuleInfo::default();
        let mut current_attr: Option<String> = None;

        let metadata = match iohe_root.info() {
            Some(info) => {
                let parser = EventReader::new(info.as_bytes());
                for e in parser {
                    match e {
                        Ok(XmlEvent::StartElement {
                            name, attributes, ..
                        }) => {
                            match name.local_name.as_str() {
                                "dv" => {}
                                "node" => {
                                    if is_root_output_node(attributes) && current_attr.is_some() {
                                        // finished parsing the module
                                        module_vector.push(current_module);
                                        current_module = ModuleInfo::default();
                                        current_attr = None;
                                    }
                                }
                                "attr" => {
                                    for attr in attributes {
                                        if attr.name.local_name == "key" {
                                            current_attr = Some(attr.value.clone());
                                        }
                                    }
                                }
                                unknown_tag => {
                                    eprintln!(
                                        "found unexpected tag: {} in xml metadata",
                                        unknown_tag
                                    );
                                }
                            }
                        }
                        Ok(XmlEvent::Characters(s)) => {
                            if let Some(attr) = current_attr.as_deref() {
                                match attr {
                                    "typeIdentifier" => {
                                        current_module.output_name = s;
                                    }
                                    "sizeX" => {
                                        current_module.output_info.size_x = s.parse().ok();
                                    }
                                    "sizeY" => {
                                        current_module.output_info.size_y = s.parse().ok();
                                    }
                                    "source" => {
                                        current_module.output_info.source = Some(s.clone());
                                    }
                                    "tsOffset" => {
                                        current_module.output_info.ts_offset = s.parse().ok();
                                    }
                                    "typeDescription" => {
                                        current_module.description = s.clone();
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(_) => {}
                        _ => {}
                    }
                }
                let compression = iohe_root.compression();
                Some(VideoMetadata {
                    output_modules: module_vector,
                    compression: compression,
                })
            }
            None => None,
        };

        println!("metadata: {:?}", metadata);

        Ok(Box::new(Aedat4Decoder {
            offset: iohe_root.data_table_offset(),
            current_timestamp: 0,
            current_frame: 1,
            buffered_reader,
            metadata: metadata,
        }))
    }
}

/// State for the AEDAT4 decoder.
struct Aedat4Decoder {
    offset: i64,
    /// The current timestamp in microseconds.
    current_timestamp: i64,
    /// The current frame number.
    current_frame: i64,
    /// The buffered reader for reading the AEDAT4 file.
    buffered_reader: BufReader<ByteSource>,
    /// The metadata for the video.
    metadata: Option<VideoMetadata>,
}

trait Aedat4DecoderExt {
    fn decompress(&self, compression: CompressionType, data: &[u8])
    -> Result<Vec<u8>, DecodeError>;
}

impl Aedat4DecoderExt for Aedat4Decoder {
    fn decompress(
        &self,
        compression: CompressionType,
        data: &[u8],
    ) -> Result<Vec<u8>, DecodeError> {
        match compression {
            CompressionType::NONE => Ok(data.to_vec()),
            CompressionType::LZ4 => {
                let mut decoder =
                    lz4::Decoder::new(Cursor::new(data)).expect("decoder failed to init");
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| DecodeError::UnexpectedError(ErrorKind::Other, e.to_string()))?;

                Ok(decompressed)
            }
            _ => unimplemented!("unsupported compression type: {:?}", compression),
        }
    }
}

#[async_trait]
impl Decoder for Aedat4Decoder {
    fn metadata(&self) -> &VideoMetadata {
        self.metadata.as_ref().unwrap()
    }

    async fn next_packet(&mut self) -> Result<Option<Packet>, DecodeError> {
        // parse packet header, this is exactly 8 bytes: https://docs.inivation.com/software/software-advanced-usage/file-formats/aedat-4.0.html

        let mut header_buffer = [0u8; 8];
        self.buffered_reader
            .read_exact(&mut header_buffer)
            .await
            .map_err(|e| DecodeError::UnexpectedError(ErrorKind::Other, e.to_string()))?;

        let header = aedat4_data_generated::PacketHeader(header_buffer);

        print!(
            "[decode] packet #{:?}, compressed size: {} bytes, ",
            header.id(),
            header.size()
        );

        let mut packet_buffer = vec![0u8; header.size() as usize];
        self.buffered_reader
            .read_exact(&mut packet_buffer)
            .await
            .map_err(|e| DecodeError::UnexpectedError(ErrorKind::Other, e.to_string()))?;

        let compresssion = self.metadata.clone().expect("expected metadata, should not be here").compression;



        let decompressed_buffer = self.decompress(
            compresssion,
            &packet_buffer,
        )?;

        let identifier = String::from_utf8_lossy(&decompressed_buffer[8..12]).to_string();

        let packet_content = PacketContent {
            id: header.id(),
            buffer: decompressed_buffer,
        };

        // Validate the identifier and map it to the appropriate packet type.
        let packet = match identifier.as_str() {
            "EVTS" => Packet::EventPacket(packet_content),
            "FRME" => Packet::FramePacket(packet_content),
            "IMUS" => Packet::ImuPacket(packet_content),
            "TRIG" => Packet::TriggerPacket(packet_content),
            _ => {
                return Err(DecodeError::UnexpectedError(
                    ErrorKind::Other,
                    format!("unknown identifier: {}", identifier),
                ));
            }
        };

        Ok(Some(packet))
    }

    async fn all_frames(&mut self) -> Result<Vec<Packet>, DecodeError> {
        let mut frames = Vec::new();
        while let Some(frame) = self.next_packet().await? {
            frames.push(frame);
        }
        Ok(frames)
    }
}
