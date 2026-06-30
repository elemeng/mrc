//! Compression detection helpers.
//!
//! Provides [`CompressionType`] and [`detect_compression`] used internally by
//! [`Reader::open`](crate::Reader::open) to auto-detect gzip/bzip2.

use crate::Error;
use std::path::Path;

/// Detected compression format of an MRC file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    let mut magic = [0u8; 2];
    let n = file.read(&mut magic)?;
    if n < 2 {
        return Ok(CompressionType::Plain);
    }

    #[cfg(feature = "gzip")]
    if magic == [0x1f, 0x8b] {
        return Ok(CompressionType::Gzip);
    }

    #[cfg(feature = "bzip2")]
    if magic == [b'B', b'Z'] {
        return Ok(CompressionType::Bzip2);
    }

    Ok(CompressionType::Plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_compression_plain() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"MAP ").unwrap();
        assert_eq!(
            detect_compression(tmp.path()).unwrap(),
            CompressionType::Plain
        );
    }

    #[test]
    #[cfg(feature = "gzip")]
    fn test_detect_compression_gzip() {
        use std::io::Write;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut file = std::fs::File::create(tmp.path()).unwrap();
        file.write_all(&[0x1f, 0x8b, 0x08, 0x00]).unwrap();
        drop(file);
        assert_eq!(
            detect_compression(tmp.path()).unwrap(),
            CompressionType::Gzip
        );
    }

    #[test]
    #[cfg(feature = "bzip2")]
    fn test_detect_compression_bzip2() {
        use std::io::Write;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut file = std::fs::File::create(tmp.path()).unwrap();
        file.write_all(b"BZh").unwrap();
        drop(file);
        assert_eq!(
            detect_compression(tmp.path()).unwrap(),
            CompressionType::Bzip2
        );
    }
}
