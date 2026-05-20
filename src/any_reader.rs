//! Unified MRC reader with automatic compression detection.
//!
//! This module provides [`MrcReader`], an enum that wraps the concrete reader
//! types ([`Reader`](crate::Reader), [`GzipReader`](crate::GzipReader),
//! [`Bzip2Reader`](crate::Bzip2Reader)) and dispatches to the correct one
//! based on file magic bytes.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::endian::FileEndian;
use crate::mode::{M0Interpretation, Voxel};
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

/// A unified MRC file reader that auto-detects compression.
///
/// Created via [`open`](crate::open) or [`MrcReader::open`].
#[derive(Debug)]
pub enum MrcReader {
    Plain(crate::Reader),
    #[cfg(feature = "gzip")]
    Gzip(crate::GzipReader),
    #[cfg(feature = "bzip2")]
    Bzip2(crate::Bzip2Reader),
}

impl MrcReader {
    /// Open an MRC file, automatically detecting gzip or bzip2 compression.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        match detect_compression(&path)? {
            CompressionType::Plain => Ok(MrcReader::Plain(crate::Reader::open(path)?)),
            #[cfg(feature = "gzip")]
            CompressionType::Gzip => Ok(MrcReader::Gzip(crate::GzipReader::open(path)?)),
            #[cfg(feature = "bzip2")]
            CompressionType::Bzip2 => Ok(MrcReader::Bzip2(crate::Bzip2Reader::open(path)?)),
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
                let (r, w) = crate::GzipReader::open_permissive(path)?;
                Ok((MrcReader::Gzip(r), w))
            }
            #[cfg(feature = "bzip2")]
            CompressionType::Bzip2 => {
                let (r, w) = crate::Bzip2Reader::open_permissive(path)?;
                Ok((MrcReader::Bzip2(r), w))
            }
        }
    }

    /// Volume dimensions.
    pub fn shape(&self) -> VolumeShape {
        match self {
            MrcReader::Plain(r) => r.shape(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.shape(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.shape(),
        }
    }

    /// Voxel data mode.
    pub fn mode(&self) -> Mode {
        match self {
            MrcReader::Plain(r) => r.mode(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.mode(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.mode(),
        }
    }

    /// Reference to the parsed header.
    pub fn header(&self) -> &Header {
        match self {
            MrcReader::Plain(r) => r.header(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.header(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.header(),
        }
    }

    /// Raw extended header bytes.
    pub fn ext_header_bytes(&self) -> &[u8] {
        match self {
            MrcReader::Plain(r) => r.ext_header_bytes(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.ext_header_bytes(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.ext_header_bytes(),
        }
    }

    /// Read a block of raw bytes.
    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        match self {
            MrcReader::Plain(r) => r.read_block_bytes(offset, shape),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.read_block_bytes(offset, shape),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.read_block_bytes(offset, shape),
        }
    }

    /// Read and decode a voxel block.
    pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error> {
        match self {
            MrcReader::Plain(r) => r.read_block(offset, shape),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.read_block(offset, shape),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.read_block(offset, shape),
        }
    }

    /// Iterate over slices, converting common types to `f32`.
    pub fn slices_f32(&self) -> Result<Box<dyn Iterator<Item = Result<VoxelBlock<f32>, Error>> + '_>, Error> {
        match self {
            MrcReader::Plain(r) => r.slices_f32(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.slices_f32(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.slices_f32(),
        }
    }

    /// Iterate over slabs, converting common types to `f32`.
    pub fn slabs_f32(&self, k: usize) -> Result<Box<dyn Iterator<Item = Result<VoxelBlock<f32>, Error>> + '_>, Error> {
        match self {
            MrcReader::Plain(r) => r.slabs_f32(k),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.slabs_f32(k),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.slabs_f32(k),
        }
    }

    /// Iterate over slices, automatically converting Mode 6 (`Uint16`) to `u8`.
    ///
    /// Returns an error if the file is not Mode 6 or if any value exceeds 255.
    pub fn slices_u8(&self) -> Result<Box<dyn Iterator<Item = Result<VoxelBlock<u8>, Error>> + '_>, Error> {
        match self {
            MrcReader::Plain(r) => Ok(Box::new(r.slices_u8()?)),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => Ok(Box::new(r.slices_u8()?)),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => Ok(Box::new(r.slices_u8()?)),
        }
    }

    /// Iterate over slices for Mode 0 with explicit signed/unsigned choice.
    pub fn slices_mode0(
        &self,
        interp: M0Interpretation,
    ) -> Result<Box<dyn Iterator<Item = Result<VoxelBlock<f32>, Error>> + '_>, Error> {
        match self {
            MrcReader::Plain(r) => {
                Ok(Box::new(r.slices_mode0(interp)))
            }
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => {
                Ok(Box::new(r.slices_mode0(interp)))
            }
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => {
                Ok(Box::new(r.slices_mode0(interp)))
            }
        }
    }

    /// Cross-check header statistics against actual data.
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        match self {
            MrcReader::Plain(r) => r.validate_header_stats(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.validate_header_stats(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.validate_header_stats(),
        }
    }

    /// Raw data bytes (after header and extended header).
    pub fn data_bytes(&self) -> &[u8] {
        match self {
            MrcReader::Plain(r) => r.data(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.data_bytes(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.data_bytes(),
        }
    }

    /// Detected file endianness.
    pub fn endian(&self) -> FileEndian {
        match self {
            MrcReader::Plain(r) => r.endian(),
            #[cfg(feature = "gzip")]
            MrcReader::Gzip(r) => r.header().detect_endian(),
            #[cfg(feature = "bzip2")]
            MrcReader::Bzip2(r) => r.header().detect_endian(),
        }
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

    /// Access the underlying plain [`Reader`](crate::Reader), if any.
    pub fn as_reader(&self) -> Option<&crate::Reader> {
        match self {
            MrcReader::Plain(r) => Some(r),
            _ => None,
        }
    }

    /// Access the underlying [`GzipReader`](crate::GzipReader), if any.
    #[cfg(feature = "gzip")]
    pub fn as_gzip_reader(&self) -> Option<&crate::GzipReader> {
        match self {
            MrcReader::Gzip(r) => Some(r),
            _ => None,
        }
    }

    /// Access the underlying [`Bzip2Reader`](crate::Bzip2Reader), if any.
    #[cfg(feature = "bzip2")]
    pub fn as_bzip2_reader(&self) -> Option<&crate::Bzip2Reader> {
        match self {
            MrcReader::Bzip2(r) => Some(r),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_detect_compression_plain() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"MAP ").unwrap();
        assert_eq!(detect_compression(tmp.path()).unwrap(), CompressionType::Plain);
    }

    #[test]
    #[cfg(feature = "gzip")]
    fn test_detect_compression_gzip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut file = std::fs::File::create(tmp.path()).unwrap();
        file.write_all(&[0x1f, 0x8b, 0x08, 0x00]).unwrap();
        drop(file);
        assert_eq!(detect_compression(tmp.path()).unwrap(), CompressionType::Gzip);
    }

    #[test]
    #[cfg(feature = "bzip2")]
    fn test_detect_compression_bzip2() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut file = std::fs::File::create(tmp.path()).unwrap();
        file.write_all(b"BZh").unwrap();
        drop(file);
        assert_eq!(detect_compression(tmp.path()).unwrap(), CompressionType::Bzip2);
    }

    #[test]
    fn test_mrc_reader_open_plain() {
        use crate::{create, VoxelBlock};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([8, 8, 2])
            .mode::<f32>()
            .finish()
            .unwrap();
        for z in 0..2 {
            let data = vec![1.0f32; 8 * 8];
            writer.write_block(&VoxelBlock::new([0, 0, z], [8, 8, 1], data)).unwrap();
        }
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        assert_eq!(reader.shape().nx, 8);
        assert_eq!(reader.mode(), crate::Mode::Float32);
    }

    #[test]
    fn test_mrc_reader_open_permissive() {
        use crate::{create, VoxelBlock};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([8, 8, 2])
            .mode::<f32>()
            .finish()
            .unwrap();
        for z in 0..2 {
            let data = vec![1.0f32; 8 * 8];
            writer.write_block(&VoxelBlock::new([0, 0, z], [8, 8, 1], data)).unwrap();
        }
        writer.finalize().unwrap();

        let (reader, warnings) = MrcReader::open_permissive(tmp.path()).unwrap();
        assert!(reader.is_plain());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_mrc_reader_type_accessors() {
        use crate::{create, VoxelBlock};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        writer.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], vec![0.0f32; 16])).unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        assert!(reader.is_plain());
        assert!(!reader.is_gzip());
        assert!(!reader.is_bzip2());
        assert!(reader.as_reader().is_some());
        assert!(reader.as_gzip_reader().is_none());
        assert!(reader.as_bzip2_reader().is_none());
    }

    #[test]
    fn test_mrc_reader_validate_header_stats() {
        use crate::{create, VoxelBlock};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        writer.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], vec![1.0f32; 16])).unwrap();
        writer.update_header_stats().unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        assert!(reader.validate_header_stats().is_ok());
    }

    #[test]
    fn test_mrc_reader_data_bytes() {
        use crate::{create, VoxelBlock};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([2, 2, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        writer.write_block(&VoxelBlock::new([0, 0, 0], [2, 2, 1], data.clone())).unwrap();
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
        use crate::{create, VoxelBlock};

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut writer = create(tmp.path())
            .shape([2, 2, 1])
            .mode::<u16>()
            .finish()
            .unwrap();
        writer.write_u8_block(&VoxelBlock::new([0, 0, 0], [2, 2, 1], vec![10u8, 20, 30, 40])).unwrap();
        writer.finalize().unwrap();

        let reader = MrcReader::open(tmp.path()).unwrap();
        let mut slices = reader.slices_u8().unwrap();
        let block = slices.next().unwrap().unwrap();
        assert_eq!(block.data, vec![10u8, 20, 30, 40]);
    }
}
