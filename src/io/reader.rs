//! Compression detection helpers.
//!
//! Provides [`CompressionType`] and [`detect_compression`] used internally by
//! [`Reader::open`](crate::Reader::open) to auto-detect gzip/bzip2.

use crate::Error;
use std::path::Path;

/// Detected compression format of an MRC file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompressionType {
    /// Uncompressed MRC file.
    Plain,
    /// Gzip-compressed MRC file.
    #[cfg(feature = "gzip")]
    Gzip,
    /// Bzip2-compressed MRC file.
    #[cfg(feature = "bzip2")]
    Bzip2,
}

/// Peek at a byte slice to determine its compression format.
///
/// | Magic bytes | Format |
/// |-------------|--------|
/// | `\x1f\x8b` | Gzip |
/// | `BZ` | Bzip2 |
/// | anything else | Plain |
///
/// Returns [`CompressionType::Plain`] if fewer than 2 bytes are provided.
#[doc(hidden)]
pub fn detect_compression_from_bytes(bytes: &[u8]) -> CompressionType {
    if bytes.len() < 2 {
        return CompressionType::Plain;
    }

    let magic = [bytes[0], bytes[1]];

    #[cfg(feature = "gzip")]
    if magic == [0x1f, 0x8b] {
        return CompressionType::Gzip;
    }

    #[cfg(feature = "bzip2")]
    if magic == [b'B', b'Z'] {
        return CompressionType::Bzip2;
    }

    CompressionType::Plain
}

/// Peek at the first bytes of a file to determine its compression format.
///
/// | Magic bytes | Format |
/// |-------------|--------|
/// | `\x1f\x8b` | Gzip |
/// | `BZ` | Bzip2 |
/// | anything else | Plain |
///
/// This performs a single small read from the beginning of the file.
pub fn detect_compression<P: AsRef<Path>>(path: P) -> Result<CompressionType, Error> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut buf = [0u8; 2];
    let n = file.read(&mut buf)?;
    Ok(detect_compression_from_bytes(&buf[..n]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_compression_plain() {
        assert_eq!(
            detect_compression_from_bytes(b"MAP "),
            CompressionType::Plain
        );
        assert_eq!(detect_compression_from_bytes(b""), CompressionType::Plain);
        assert_eq!(
            detect_compression_from_bytes(b"\x00\x00"),
            CompressionType::Plain
        );
    }

    #[test]
    #[cfg(feature = "gzip")]
    fn test_detect_compression_gzip() {
        assert_eq!(
            detect_compression_from_bytes(&[0x1f, 0x8b, 0x08, 0x00]),
            CompressionType::Gzip
        );
    }

    #[test]
    #[cfg(feature = "bzip2")]
    fn test_detect_compression_bzip2() {
        assert_eq!(
            detect_compression_from_bytes(b"BZh"),
            CompressionType::Bzip2
        );
    }
}
