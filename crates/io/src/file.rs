/// Represents a file format
#[derive(Debug, Clone, Copy)]
pub struct FileFormat {
    /// Human-readable name of the file format
    pub name: &'static str,
    /// Magic bytes used to validate the file
    pub magic_bytes: Option<&'static [u8]>,
    /// File extension associated with the codec.
    pub extension: &'static str,
}

pub static AEDAT4_FORMAT: FileFormat = FileFormat {
    name: "AEDAT 4",
    magic_bytes: Some(b"#!AER-DAT4.0\r\n"),
    extension: "aedat4",
};
