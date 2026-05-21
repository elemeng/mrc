//! In-memory buffered MRC file reader.
//!
//! Provides [`Reader`], which loads the entire file into a `Vec<u8>` on open.
//! This enables fast random access to any slice or block, but requires enough
//! RAM to hold the full dataset.

use crate::engine::block::VolumeShape;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::vec::Vec;

/// In-memory buffered MRC file reader.
///
/// The entire file is read into memory on open, making this suitable for
/// smaller files or when random access to any slice is needed.
///
/// For large files that don't fit in RAM, consider [`MmapReader`](crate::MmapReader)
/// (requires the `mmap` feature).
///
/// # Example
///
/// ```no_run
/// use mrc::{Reader, ReaderExt};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let reader = Reader::open("protein.mrc")?;
///     for slice in reader.slices::<f32>() {
///         let block = slice?;
///         // block.data is Vec<f32>
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Reader {
    pub(crate) header: Header,
    pub(crate) ext_header: Vec<u8>,
    pub(crate) data: Vec<u8>,
    pub(crate) endian: FileEndian,
    pub(crate) shape: VolumeShape,
}

impl Reader {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Self::_open(path, false).map(|(r, _)| r)
    }

    /// Open an MRC file in **permissive** mode.
    ///
    /// Non-fatal header issues (unusual MAP field, unexpected `nversion`,
    /// non-standard axis mapping, etc.) are collected as warning strings
    /// instead of causing a hard error. Only genuinely unreadable files
    /// (negative dimensions, unsupported mode, IO failure) return `Err`.
    pub fn open_permissive<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open(path, true)
    }

    fn _open<P: AsRef<std::path::Path>>(
        path: P,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;

        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let (header, warnings, endian, data_size) =
            crate::io::reader_common::parse_header(&header_bytes, permissive)?;

        let ext_size = header.nsymbt as usize;
        let mut ext_header = vec![0u8; ext_size];
        if ext_size > 0 {
            file.read_exact(&mut ext_header)?;
        }

        let mut data = vec![0u8; data_size];
        file.read_exact(&mut data)?;

        if !permissive {
            let file_len = file.metadata()?.len() as usize;
            let expected_len = header.data_offset() + data_size;
            if file_len != expected_len {
                return Err(Error::FileSizeMismatch {
                    expected: expected_len,
                    actual: file_len,
                });
            }
        }

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok((
            Self {
                header,
                ext_header,
                data,
                endian,
                shape,
            },
            warnings,
        ))
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Read a block of raw voxel bytes from the file.
    ///
    /// Supports arbitrary sub-blocks; non-contiguous regions are gathered
    /// into a contiguous buffer automatically.
    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        crate::io::reader_common::validate_block_bounds(
            self.shape,
            self.mode(),
            self.data.len(),
            offset,
            shape,
        )?;
        Ok(crate::io::reader_common::gather_block_bytes(
            &self.data,
            self.shape,
            self.mode(),
            offset,
            shape,
        ))
    }

    /// Read and decode a block of voxels to the specified type.
    ///
    /// Returns an error if `T` does not match the file's voxel mode.
    pub fn read_block<T: Voxel>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<crate::engine::block::VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(crate::engine::block::VoxelBlock {
            offset,
            shape,
            data,
        })
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// # Errors
    /// Returns `Error::ModeMismatch` if `T` does not match the file mode.
    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        crate::io::reader_common::decode_block(bytes, self.mode(), self.endian)
    }

}

impl Reader {
    /// Get a reference to the raw data bytes
    pub fn data_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Cross-check header statistics against actual data.
    ///
    /// Computes `dmin`, `dmax`, `dmean` and `rms` from the data block and
    /// compares them with the header values using a 1 % relative tolerance
    /// (matching Python `mrcfile`'s `np.isclose(rtol=0.01)`).
    ///
    /// # Errors
    /// Returns [`Error::StatsMismatch`] if any statistic deviates by more than 1 %.
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        crate::engine::stats::validate_header_stats(&self.header, &self.data)
    }

    /// Get the file endianness
    pub fn endian(&self) -> FileEndian {
        self.endian
    }

    /// Get the extended header bytes, if any.
    pub fn ext_header_bytes(&self) -> &[u8] {
        &self.ext_header
    }
}
