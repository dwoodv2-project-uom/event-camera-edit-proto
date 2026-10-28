use std::error::Error as _;
use std::io::{ErrorKind, Read as _, SeekFrom};

use async_trait::async_trait;
///
/// Decoder for the AEDAT4 file format decribed [here](https://docs.inivation.com/software/software-advanced-usage/file-formats/aedat-4.0.html).
///
use tokio::io::{self, AsyncReadExt as _, AsyncSeekExt as _, BufReader};

use xml::{
    attribute::OwnedAttribute,
    reader::{EventReader, XmlEvent},
};

use crate::codec::PacketType;
use crate::file_data_table::{PacketHeader, size_prefixed_root_as_file_data_table};
use crate::utils::decompress;
use crate::{
    aedat4_data_generated,
    aedat4_header_generated::{self, CompressionType},
    codec::{Codec, Decoder, DecoderFactory, ModuleInfo, Packet, PacketContent, VideoMetadata},
    error::{DecodeError, FileReadError},
    file::{AEDAT4_FORMAT, FileFormat},
};
use tokio::fs::File;

/// Struct representing the state of AEDAT4 codec.
pub struct Aedat4;

#[derive(Debug, Clone)]
pub struct PacketMetadata {
    header: PacketHeader,
    byte_offset: i64,
    num_elements: i64,
    ts_start: i64,
    ts_end: i64,
}

#[derive(Debug, Clone)]
pub struct FileDataTableEntry {
    packet_type: PacketType,
    metadata: PacketMetadata,
}

#[derive(Debug, Clone)]
pub struct FileDataTable {
    pub total_events: i64,
    pub total_frames: i64,
    pub total_imu: i64,
    pub total_trig: i64,
    pub ts_start: i64,
    pub ts_end: i64,
    pub events: Vec<FileDataTableEntry>,
    pub frames: Vec<FileDataTableEntry>,
    pub imus: Vec<FileDataTableEntry>,
    pub trigs: Vec<FileDataTableEntry>,
}

/// Wrapper around the `FileDataTable` which allows for threadsafe access of the `FileDataTable`
impl FileDataTable {
    /// Creates a new `FileDataTable` from the given bytes.
    /// We have to do some extra reading, as we need to determine the type of the packet (therefore have to read)
    pub async fn new(
        reader: &mut BufReader<File>,
        decompressed_bytes: Vec<u8>,
        compression_type: CompressionType,
    ) -> Result<Self, DecodeError> {
        // Get the original file data table from the file
        let file_data_table_fb = size_prefixed_root_as_file_data_table(&decompressed_bytes)
            .map_err(|e| {
                DecodeError::UnexpectedError(
                    ErrorKind::InvalidData,
                    String::from("flatbuffer was malformed: ") + &e.to_string(),
                )
            })?;

        let mut events = Vec::new();
        let mut frames = Vec::new();
        let mut imus = Vec::new();
        let mut trigs = Vec::new();

        let mut total_events = 0;
        let mut total_frames = 0;
        let mut total_imu = 0;
        let mut total_trig = 0;
        let mut ts_start = i64::MAX;
        let mut ts_end = i64::MIN;

        let fb_table = file_data_table_fb.table().unwrap();

        for element in fb_table {
            let header = element.header().unwrap();

            let metadata = PacketMetadata {
                header: *header,
                byte_offset: element.byte_offset(),
                num_elements: element.num_elements(),
                ts_start: element.ts_start(),
                ts_end: element.ts_end(),
            };

            ts_start = ts_start.min(metadata.ts_start);
            ts_end = ts_end.max(metadata.ts_end);

            let packet = read_packet(
                reader,
                compression_type,
                element.byte_offset() as u64,
                *element.header().unwrap(),
            )
            .await?;

            match packet {
                Packet::EventPacket(_) => {
                    events.push(FileDataTableEntry {
                        packet_type: PacketType::EventPacket,
                        metadata,
                    });
                    total_events += element.num_elements();
                }
                Packet::FramePacket(_) => {
                    frames.push(FileDataTableEntry {
                        packet_type: PacketType::FramePacket,
                        metadata,
                    });
                    total_frames += element.num_elements();
                }
                Packet::ImuPacket(_) => {
                    imus.push(FileDataTableEntry {
                        packet_type: PacketType::ImuPacket,
                        metadata,
                    });
                    total_imu += element.num_elements();
                }
                Packet::TriggerPacket(_) => {
                    trigs.push(FileDataTableEntry {
                        packet_type: PacketType::TriggerPacket,
                        metadata,
                    });
                    total_trig += element.num_elements();
                }
            }
        }

        Ok(Self {
            total_events,
            total_frames,
            total_imu,
            total_trig,
            ts_start,
            ts_end,
            events,
            frames,
            imus,
            trigs,
        })
    }
}

async fn read_exact_and_handle_err(
    buffered_reader: &mut BufReader<File>,
    buffer: &mut [u8],
) -> Result<(), FileReadError> {
    match buffered_reader
        .read_exact(buffer)
        .await
        .map_err(|err| match err.kind() {
            io::ErrorKind::UnexpectedEof => FileReadError::UnexpectedEOF(
                "unexpected EOF: file ended before magic bytes ended".to_owned(),
            ),
            _ => FileReadError::UnexpectedError(
                io::ErrorKind::Other,
                format!("unexpected I/O error: {err}"),
            ),
        }) {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}
async fn read_packet(
    reader: &mut BufReader<File>,
    compression_type: CompressionType,
    offset: u64,
    header: PacketHeader,
) -> Result<Packet, DecodeError> {
    reader
        .seek(SeekFrom::Start(offset))
        .await
        .map_err(|e| DecodeError::UnexpectedError(ErrorKind::UnexpectedEof, e.to_string()))?;

    let mut header_bytes = vec![0; header.size() as usize];

    reader
        .read_exact(&mut header_bytes)
        .await
        .map_err(|e| DecodeError::UnexpectedError(ErrorKind::UnexpectedEof, e.to_string()))?;

    let decompressed_bytes = decompress(compression_type, &header_bytes).map_err(|_e| {
        DecodeError::UnexpectedError(ErrorKind::UnexpectedEof, "decompression failed".to_owned())
    })?;

    if decompressed_bytes.len() < 8 {
        return Err(DecodeError::UnexpectedError(
            ErrorKind::UnexpectedEof,
            "found EOF before flatbuffer data started ".to_owned(),
        ));
    }
    // non-size prefixed bytes: root_table_offset[0:3], identifier [4:7], data [8:]
    // size prefixed bytes: root_table_offset[0:3], size prefix [4:7], identifier [8:12], data [12:]
    let bytes_4_7 = String::from_utf8_lossy(&decompressed_bytes[4..7]).to_string();
    let packet_type = bytes_4_7.parse::<PacketType>().or_else(|_err| {
        let bytes_8_12 = String::from_utf8_lossy(&decompressed_bytes[8..12]).to_string();
        bytes_8_12.parse::<PacketType>()
    });

    if packet_type.is_err() {
        return Err(DecodeError::UnexpectedError(
            ErrorKind::Other,
            "failed to parse packet type".to_owned(),
        ));
    }

    let packet_type = packet_type.unwrap();
    let content = PacketContent {
        id: header.id(),
        buffer: decompressed_bytes,
    };

    match packet_type {
        PacketType::EventPacket => Ok(Packet::EventPacket(content)),
        PacketType::FramePacket => Ok(Packet::FramePacket(content)),
        PacketType::ImuPacket => Ok(Packet::ImuPacket(content)),
        PacketType::TriggerPacket => Ok(Packet::TriggerPacket(content)),
    }
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
    type Decoder = Aedat4Decoder;
    async fn open(&self, source: File) -> Result<Box<Aedat4Decoder>, FileReadError> {
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

        if iohe_root.data_table_offset() == -1 {
            return Err(FileReadError::UnexpectedError(
                ErrorKind::InvalidData,
                String::from("File must contain file data table, this is for indexing purposes."),
            ));
        }

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
                                        "found unexpected tag: {unknown_tag} in xml metadata"
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
                    compression,
                })
            }
            None => None,
        };

        //println!("metadata: {metadata:?}");

        // read the data table, this contains indexes for each packet
        buffered_reader
            .seek(SeekFrom::Start(iohe_root.data_table_offset() as u64))
            .await
            .expect("TODO: panic message");

        let mut data_table_buffer = Vec::new();
        buffered_reader
            .read_to_end(&mut data_table_buffer)
            .await
            .expect("TODO: panic message");

        let compresssion = metadata
            .clone()
            .expect("expected metadata, should not be here")
            .compression;

        let decompressed_buffer =
            crate::utils::decompress(compresssion, &data_table_buffer).unwrap();

        let file_data_table =
            FileDataTable::new(&mut buffered_reader, decompressed_buffer, compresssion)
                .await
                .map_err(|_e| {
                    FileReadError::UnexpectedError(
                        ErrorKind::InvalidData,
                        "Failed to parse file data table".to_owned(),
                    )
                })?;

        let decoder = Aedat4Decoder {
            file_data_table,
            buffered_reader,
            metadata,
        };

        //println!("file_data_table: {:?}", decoder.file_data_table);

        Ok(Box::new(decoder))
    }
}

/// State for the AEDAT4 decoder.
pub struct Aedat4Decoder {
    pub file_data_table: FileDataTable,
    /// The buffered reader for reading the AEDAT4 file.
    pub buffered_reader: BufReader<File>,
    /// The metadata for the video.
    pub metadata: Option<VideoMetadata>,
}

#[async_trait]
impl Decoder for Aedat4Decoder {
    fn metadata(&self) -> &VideoMetadata {
        self.metadata.as_ref().unwrap()
    }

    /// Returns a vector of packets that fall within the given timestamp range.
    /// Complexity is O(log n)
    async fn packets_by_ts_range(
        &mut self,
        t1: i64,
        t2: i64,
    ) -> Result<Option<Vec<Packet>>, DecodeError> {
        if t1 > t2 {
            return Ok(None);
        }

        fn find_bounds(arr: &[FileDataTableEntry], t1: i64, t2: i64) -> (usize, usize) {
            let lower_bound_idx = arr.partition_point(|e| e.metadata.ts_start < t1);
            let upper_bound_idx = arr.partition_point(|e| e.metadata.ts_end <= t2);

            (lower_bound_idx, upper_bound_idx)
        }

        let events = &self.file_data_table.events;
        assert!(events.iter().map(|e| e.metadata.ts_start).is_sorted());
        assert!(events.iter().map(|e| e.metadata.ts_end).is_sorted());

        let (lower_bound_idx, upper_bound_idx) = find_bounds(events, t1, t2);
        let filtered_events = events
            .get(lower_bound_idx..upper_bound_idx)
            .unwrap_or_default();

        let frames = &self.file_data_table.frames;
        assert!(frames.iter().map(|e| e.metadata.ts_start).is_sorted());
        assert!(frames.iter().map(|e| e.metadata.ts_end).is_sorted());

        let (lower_bound_idx, upper_bound_idx) = find_bounds(frames, t1, t2);
        let filtered_frames = frames
            .get(lower_bound_idx..upper_bound_idx)
            .unwrap_or_default();

        let imus = &self.file_data_table.imus;
        assert!(imus.iter().map(|e| e.metadata.ts_start).is_sorted());
        assert!(imus.iter().map(|e| e.metadata.ts_end).is_sorted());

        let (lower_bound_idx, upper_bound_idx) = find_bounds(imus, t1, t2);
        let filtered_imus = imus
            .get(lower_bound_idx..upper_bound_idx)
            .unwrap_or_default();

        let trigs = &self.file_data_table.trigs;
        assert!(trigs.iter().map(|e| e.metadata.ts_start).is_sorted());
        assert!(trigs.iter().map(|e| e.metadata.ts_end).is_sorted());

        let (lower_bound_idx, upper_bound_idx) = find_bounds(trigs, t1, t2);
        let filtered_trigs = trigs
            .get(lower_bound_idx..upper_bound_idx)
            .unwrap_or_default();

        let to_read = vec![
            filtered_events,
            filtered_frames,
            filtered_imus,
            filtered_trigs,
        ];

        let mut packets = Vec::new();

        for packet_type in to_read {
            for packet in packet_type {
                let packet = read_packet(
                    &mut self.buffered_reader,
                    self.metadata
                        .as_ref()
                        .expect("expected metadata, should not be here")
                        .compression,
                    packet.metadata.byte_offset as u64,
                    packet.metadata.header,
                )
                .await?;
                packets.push(packet);
            }
        }

        Ok(Some(packets))
    }

    async fn next_packet(&mut self) -> Result<Option<Packet>, DecodeError> {
        // parse packet header, this is exactly 8 bytes: https://docs.inivation.com/software/software-advanced-usage/file-formats/aedat-4.0.html

        let mut header_buffer = [0u8; 8];
        self.buffered_reader
            .read_exact(&mut header_buffer)
            .await
            .map_err(|e| DecodeError::UnexpectedError(ErrorKind::Other, e.to_string()))?;

        let header = aedat4_data_generated::PacketHeader(header_buffer);

        /*
        print!(
            "[decode] packet #{:?}, compressed size: {} bytes, ",
            header.id(),
            header.size()
        );*/

        let mut packet_buffer = vec![0u8; header.size() as usize];
        self.buffered_reader
            .read_exact(&mut packet_buffer)
            .await
            .map_err(|e| DecodeError::UnexpectedError(ErrorKind::Other, e.to_string()))?;

        let compresssion = self
            .metadata
            .clone()
            .expect("expected metadata, should not be here")
            .compression;

        let decompressed_buffer = decompress(compresssion, &packet_buffer)?;

        let identifier = String::from_utf8_lossy(&decompressed_buffer[8..12]).to_string();

        let packet_content = PacketContent {
            id: header.id(),
            buffer: decompressed_buffer,
        };

        let packet = match identifier.parse::<PacketType>()? {
            PacketType::EventPacket => Packet::EventPacket(packet_content),
            PacketType::FramePacket => Packet::FramePacket(packet_content),
            PacketType::ImuPacket => Packet::ImuPacket(packet_content),
            PacketType::TriggerPacket => Packet::TriggerPacket(packet_content),
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

/// Turns (uncompressed) raw bytes into flatbuffer views so the data can be used in other functions
/// TODO: we can probably use unsafe version as the packets were already verified when the file data table was made
impl PacketContent {
    pub fn as_events(
        &self,
    ) -> Result<Option<flatbuffers::Vector<'_, crate::event::Event>>, flatbuffers::InvalidFlatbuffer>
    {
        let packet_view = crate::event::size_prefixed_root_as_event_packet(&self.buffer)?;
        Ok(packet_view.elements())
    }

    pub fn as_frame(&self) -> Result<crate::frame::Frame<'_>, flatbuffers::InvalidFlatbuffer> {
        crate::frame::size_prefixed_root_as_frame(&self.buffer)
    }

    pub fn as_imu(&self) -> Result<crate::imu::IMU<'_>, flatbuffers::InvalidFlatbuffer> {
        flatbuffers::size_prefixed_root::<crate::imu::IMU<'_>>(&self.buffer)
    }

    pub fn as_trigger(
        &self,
    ) -> Result<crate::trigger::Trigger<'_>, flatbuffers::InvalidFlatbuffer> {
        flatbuffers::size_prefixed_root::<crate::trigger::Trigger<'_>>(&self.buffer)
    }
}
