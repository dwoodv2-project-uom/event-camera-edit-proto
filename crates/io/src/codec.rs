use async_trait::async_trait;
use std::{pin::Pin, str::FromStr};
use tokio::io::AsyncRead;

use crate::{
    aedat4_header_generated::CompressionType,
    error::{DecodeError, FileReadError},
    event,
    file::FileFormat,
    frame::{self, Frame},
    imu, trigger,
};

/// (asyncronous) source of bytes, e.g. a file, network stream etc.
pub type ByteSource = Pin<Box<dyn AsyncRead + Send>>;

/// Represents a codec which decodes / encodes a video file.
pub trait Codec {
    /// Returns the file format of the input file.
    fn format(&self) -> &FileFormat;
    /// Validates the input data against the codec's file format.
    /// `data` should not be the entire file in bytes, but preferably just the header.
    fn validate(&self, data: &[u8]) -> Result<(), FileReadError>;
}

/// Creates a decoder for a codec with a decoder implementation.
pub trait DecoderFactory: Codec {
    /// Opens and validates a stream of bytes, e.g. file, network stream, etc.
    /// It does not decode the entire file, responsibility is delegated to the decoder.
    async fn open(&self, source: ByteSource) -> Result<Box<dyn Decoder>, FileReadError>;
}

/// Represents a decoder which decodes a video file. Basically just a fancy iterator over the decoded frames.

#[async_trait]
pub trait Decoder {
    /// Get metadata about the decoded video.
    fn metadata(&self) -> &VideoMetadata;

    /// Decode the next frame.
    async fn next_packet(&mut self) -> Result<Option<Packet>, DecodeError>;

    /// Decode all frames and return them as a vector. Very slow!!!
    async fn all_frames(&mut self) -> Result<Vec<Packet>, DecodeError>;
}

//pub trait Encoder {}

impl FromStr for CompressionType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(CompressionType::NONE),
            "lz4" => Ok(CompressionType::LZ4),
            "lz4_high" => Ok(CompressionType::LZ4_HIGH),
            "zstd" => Ok(CompressionType::ZSTD),
            "zstd_high" => Ok(CompressionType::ZSTD_HIGH),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    // A vector containing information about the output modules
    pub output_modules: Vec<ModuleInfo>,
    /// The compression type used for all streams
    pub compression: CompressionType,
}

#[derive(Debug, Clone)]
pub struct OutputInfo {
    /// The width of the output in pixels (if applicable)
    pub size_x: Option<u32>,
    /// The height of the output in pixels (if applicable)
    pub size_y: Option<u32>,
    /// The source of the output, e.g. a specific camera name: DAVIS346_00000002
    pub source: Option<String>,
    /// The timestamp offset of the output
    pub ts_offset: Option<i64>,
}

#[derive(Debug, Clone)]
/// Information about the module that generated this output e.g. a module which records events
pub struct ModuleInfo {
    /// Name of the output produced by the module, e.g events, IMU, frames
    pub output_name: String,
    /// Description of the module, e.g. "a standard 8-bit image"
    pub description: String,
    /// Metadata about the output, e.g. compression type, dimensions
    pub output_info: OutputInfo,
}

impl Default for ModuleInfo {
    fn default() -> Self {
        Self {
            description: String::new(),
            output_name: String::new(),
            output_info: OutputInfo {
                size_x: None,
                size_y: None,
                source: None,
                ts_offset: None,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PacketContent {
    pub id: i32,
    pub buffer: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum Packet {
    EventPacket(PacketContent),
    FramePacket(PacketContent),
    ImuPacket(PacketContent),
    TriggerPacket(PacketContent),
}
