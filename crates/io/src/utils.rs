use std::io::{Cursor, ErrorKind, Read as _};

use crate::{aedat4_header_generated::CompressionType, error::DecodeError};

/// Decompresses a vector of bytes using the specified compression type.
pub fn decompress(compression: CompressionType, data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    match compression {
        CompressionType::NONE => Ok(data.to_vec()),
        CompressionType::LZ4 => {
            let mut decoder = lz4::Decoder::new(Cursor::new(data)).expect("decoder failed to init");
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| DecodeError::UnexpectedError(ErrorKind::Other, e.to_string()))?;

            Ok(decompressed)
        }
        _ => unimplemented!("unsupported compression type: {:?}", compression),
    }
}
