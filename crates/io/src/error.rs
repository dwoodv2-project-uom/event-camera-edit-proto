// Error enums

use std::io::ErrorKind;

/// Represents an error that occurred during file I/O.
#[derive(Debug)] // derive Debug so we can print the error
pub enum IOError {
    /// The file could not be found.
    FileNotFound,
    /// The file could not be read.
    FileReadError,
}

/// Represents an error that occurred during file reading.
#[derive(Debug)]
pub enum FileReadError {
    /// The file could not be parsed due to an unexpected end of file.
    UnexpectedEOF(String),
    /// The file could not be parsed due to an invalid magic bytes.
    InvalidMagicBytes,
    /// The file could not be parsed due to an unexpected error.
    UnexpectedError(ErrorKind, String),
}

#[derive(Debug)]
pub enum DecodeError {
    /// The file could not be parsed due to an unexpected error.
    UnexpectedError(ErrorKind, String),
}
