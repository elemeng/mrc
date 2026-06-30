//! Unified MRC reader with automatic compression detection.
//!
//! This module provides [`MrcReader`], an enum that wraps a [`Reader`](crate::Reader)
//! and dispatches to the correct open method based on file magic bytes.
//! Also provides the [`CompressionType`] enum and [`detect_compression`] helper.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};
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
        // File too short to have magic bytes — treat as plain MRC
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

/// Unified MRC file reader that auto-detects compression.
///
/// This enum is the recommended entry point for reading MRC files when you do
/// not know in advance whether the file is plain, gzip-compressed, or
/// bzip2-compressed. It peeks at the first two bytes to decide, then delegates
/// to the appropriate open method.
///
/// Created via [`open`](crate::open) or [`MrcReader::open`].
#[derive(Debug)]
pub enum MrcReader {
    Plain(crate::Reader),
    #[cfg(feature = "gzip")]
    Gzip(crate::Reader),
    #[cfg(feature = "bzip2")]
    Bzip2(crate::Reader),
}

macro_rules! delegate_to_reader {
    (
        $(
            $(#[$meta:meta])*
            $vis:vis fn $name:ident $(< $($gen:ident $(: $gen_bound:path)?),+ >)?
            (&self $(, $arg:ident: $ty:ty)*) $(-> $ret:ty)?;
        )+
    ) => {
        $(
            $(#[$meta])*
            $vis fn $name $(< $($gen $(: $gen_bound)?),+ >)?
            (&self $(, $arg: $ty)*) $(-> $ret)? {
                match self {
                    MrcReader::Plain(r) => r.$name($($arg),*),
                    #[cfg(feature = "gzip")]
                    MrcReader::Gzip(r) => r.$name($($arg),*),
                    #[cfg(feature = "bzip2")]
                    MrcReader::Bzip2(r) => r.$name($($arg),*),
                }
            }
        )+
    };
}

impl MrcReader {
    /// Open an MRC file, automatically detecting gzip or bzip2 compression.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        match detect_compression(&path)? {
            CompressionType::Plain => Ok(MrcReader::Plain(crate::Reader::open(path)?)),
            #[cfg(feature = "gzip")]
            CompressionType::Gzip => Ok(MrcReader::Gzip(crate::Reader::open_gzip(path)?)),
            #[cfg(feature = "bzip2")]
            CompressionType::Bzip2 => Ok(MrcReader::Bzip2(crate::Reader::open_bzip2(path)?)),
        }
    }

    /// Open in permissive mode (collects non-fatal warnings).
    ///
    /// Returns the reader together with any warnings. Fatal errors still
    /// produce `Err`.
    pub fn open_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        match detect_compression(&path)? {
            CompressionType::Plain => {
                let (r, w) = crate::Reader::open_permissive(path)?;
                Ok((MrcReader::Plain(r), w))
            }
            #[cfg(feature = "gzip")]
            CompressionType::Gzip => {
                let (r, w) = crate::Reader::open_gzip_permissive(path)?;
                Ok((MrcReader::Gzip(r), w))
            }
            #[cfg(feature = "bzip2")]
            CompressionType::Bzip2 => {
                let (r, w) = crate::Reader::open_bzip2_permissive(path)?;
                Ok((MrcReader::Bzip2(r), w))
            }
        }
    }

    delegate_to_reader! {
        /// Volume dimensions.
        pub fn shape(&self) -> VolumeShape;

        /// Voxel data mode.
        pub fn mode(&self) -> Mode;

        /// Reference to the parsed header.
        pub fn header(&self) -> &Header;

        /// Raw extended header bytes.
        pub fn ext_header_bytes(&self) -> &[u8];

        /// Read a block of raw bytes.
        pub fn read_block_bytes(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<Vec<u8>, Error>;

        /// Read and decode a voxel block.
        pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error>;

        /// Cross-check header statistics against actual data.
        pub fn validate_header_stats(&self) -> Result<(), Error>;

        /// Raw data bytes (after header and extended header).
        pub fn data_bytes(&self) -> &[u8];

        /// Detected file endianness.
        pub fn endian(&self) -> FileEndian;
    }

    // -------------------------------------------------------------------------
    // Type introspection
    // -------------------------------------------------------------------------

    /// Returns `true` if this is an uncompressed (plain) MRC file.
    pub fn is_plain(&self) -> bool {
        matches!(self, MrcReader::Plain(_))
    }

    /// Returns `true` if this is a gzip-compressed MRC file.
    #[cfg(feature = "gzip")]
    pub fn is_gzip(&self) -> bool {
        matches!(self, MrcReader::Gzip(_))
    }

    /// Returns `true` if this is a bzip2-compressed MRC file.
    #[cfg(feature = "bzip2")]
    pub fn is_bzip2(&self) -> bool {
        matches!(self, MrcReader::Bzip2(_))
    }

    /// Access the underlying [`Reader`](crate::Reader).
    #[allow(unreachable_patterns)]
    pub fn as_reader(&self) -> Option<&crate::Reader> {
        match self {
            MrcReader::Plain(r) => Some(r),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => Some(r),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => Some(r),
            _ => None,
        }
    }

    /// Access the underlying [`Reader`](crate::Reader) if this is a gzip-compressed file.
    #[cfg(feature = "gzip")]
    #[deprecated(note = "all variants now wrap Reader directly; use as_reader() instead")]
    pub fn as_gzip_reader(&self) -> Option<&crate::Reader> {
        match self {
            MrcReader::Gzip(r) => Some(r),
            _ => None,
        }
    }

    /// Access the underlying [`Reader`](crate::Reader) if this is a bzip2-compressed file.
    #[cfg(feature = "bzip2")]
    #[deprecated(note = "all variants now wrap Reader directly; use as_reader() instead")]
    pub fn as_bzip2_reader(&self) -> Option<&crate::Reader> {
        match self {
            MrcReader::Bzip2(r) => Some(r),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::reader_common::ReaderExt;

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

    #[test]
    fn test_mrc_reader_open_plain() {
        use crate::{VoxelBlock, create};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([8, 8, 2])
            .mode::<f32>()
            .finish()
            .unwrap();
        for z in 0..2 {
            let data = vec![1.0f32; 8 * 8];
            writer
                .write_block(&VoxelBlock::new([0, 0, z], [8, 8, 1], data).unwrap())
                .unwrap();
        }
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        assert_eq!(reader.shape().nx, 8);
        assert_eq!(reader.mode(), crate::Mode::Float32);
    }

    #[test]
    fn test_mrc_reader_open_permissive() {
        use crate::{VoxelBlock, create};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([8, 8, 2])
            .mode::<f32>()
            .finish()
            .unwrap();
        for z in 0..2 {
            let data = vec![1.0f32; 8 * 8];
            writer
                .write_block(&VoxelBlock::new([0, 0, z], [8, 8, 1], data).unwrap())
                .unwrap();
        }
        writer.finalize().unwrap();

        let (reader, warnings) = MrcReader::open_permissive(tmp.path()).unwrap();
        assert!(reader.is_plain());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_mrc_reader_type_accessors() {
        use crate::{VoxelBlock, create};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        writer
            .write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], vec![0.0f32; 16]).unwrap())
            .unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        assert!(reader.is_plain());
        #[cfg(feature = "gzip")]
        assert!(!reader.is_gzip());
        #[cfg(feature = "bzip2")]
        assert!(!reader.is_bzip2());
        assert!(reader.as_reader().is_some());
    }

    #[test]
    fn test_mrc_reader_validate_header_stats() {
        use crate::{VoxelBlock, create};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        writer
            .write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], vec![1.0f32; 16]).unwrap())
            .unwrap();
        writer.update_header_stats().unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        assert!(reader.validate_header_stats().is_ok());
    }

    #[test]
    fn test_mrc_reader_data_bytes() {
        use crate::{VoxelBlock, create};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([2, 2, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        writer
            .write_block(&VoxelBlock::new([0, 0, 0], [2, 2, 1], data.clone()).unwrap())
            .unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        let bytes = reader.data_bytes();
        assert_eq!(bytes.len(), 4 * 4); // 4 f32 values
        // Verify little-endian float bytes
        let expected: Vec<u8> = data.iter().flat_map(|&v| v.to_le_bytes()).collect();
        assert_eq!(bytes, expected.as_slice());
    }

    #[test]
    fn test_mrc_reader_slices_u8() {
        use crate::{VoxelBlock, create};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([2, 2, 1])
            .mode::<u16>()
            .finish()
            .unwrap();
        writer
            .write_u8_block(&VoxelBlock::new(
                [0, 0, 0],
                [2, 2, 1],
                vec![10u8, 20, 30, 40],
            ).unwrap())
            .unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        let mut slices = reader.slices_u8();
        let block = slices.next().unwrap().unwrap();
        assert_eq!(block.data, vec![10u8, 20, 30, 40]);
    }
}
