//! Memory-mapped MRC file reader with zero-copy API

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::endian::FileEndian;
use crate::iter::{BlockIter, SliceIter, SlabIter};
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::vec::Vec;

/// Memory-mapped MRC file reader.
///
/// Provides zero-copy access to MRC files by memory-mapping them into the process address space.
/// This is ideal for reading large files that don't fit in RAM, as the OS handles paging.
///
/// # Example
/// ```no_run
/// use mrc::MmapReader;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let reader = MmapReader::open("large_file.mrc")?;
///     println!("Dimensions: {:?}", reader.shape());
///
///     // Iterate over slices with zero-copy when file type matches
///     for slice in reader.slices::<f32>() {
///         let block = slice?;
///         // process block.data
///     }
///     Ok(())
/// }
/// ```
#[cfg(feature = "mmap")]
#[derive(Debug)]
pub struct MmapReader {
    mmap: memmap2::Mmap,
    header: Header,
    data_offset: usize,
    endian: FileEndian,
    shape: VolumeShape,
}

#[cfg(feature = "mmap")]
impl MmapReader {
    /// Open an MRC file via memory mapping.
    ///
    /// The file is mapped read-only into the process address space.
    /// The OS will page data in/out as needed, making this efficient for large files.
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Self::_open(path, false).map(|(r, _)| r)
    }

    /// Open an MRC file via memory mapping in **permissive** mode.
    ///
    /// Non-fatal header issues are collected as warning strings instead of
    /// causing hard errors.
    pub fn open_permissive<P: AsRef<std::path::Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        Self::_open(path, true)
    }

    fn _open<P: AsRef<std::path::Path>>(path: P, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;

        // Read header first (not mapped, since we need to parse it)
        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let (header, endian_warning) = Header::decode_from_bytes_with_info(&header_bytes);

        let mut warnings = if permissive {
            header.validate_permissive().map_err(Error::InvalidHeaderDetailed)?
        } else {
            header.validate_detailed().map_err(Error::InvalidHeaderDetailed)?;
            Vec::new()
        };

        if let Some(msg) = endian_warning {
            warnings.push(msg.to_string());
        }

        // Map the entire file
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&file)
                .map_err(|_| Error::Mmap)?
        };

        let endian = header.detect_endian();
        let shape =
            VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        // Validate file size
        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let expected_size = header.data_offset()
            .checked_add(data_size)
            .ok_or(Error::InvalidHeader)?;
        if !permissive {
            if mmap.len() != expected_size {
                return Err(Error::FileSizeMismatch {
                    expected: expected_size,
                    actual: mmap.len(),
                });
            }
        } else if mmap.len() < header.data_offset() {
            return Err(Error::FileSizeMismatch {
                expected: header.data_offset(),
                actual: mmap.len(),
            });
        }

        let _mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        Ok((Self {
            mmap,
            header,
            data_offset: header.data_offset(),
            endian,
            shape,
        }, warnings))
    }

    /// Get the volume shape (dimensions).
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Get the voxel mode (data type) of the file.
    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    /// Get a reference to the header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the file endianness.
    pub fn endian(&self) -> FileEndian {
        self.endian
    }

    /// Get the extended header bytes, if any.
    pub fn ext_header_bytes(&self) -> &[u8] {
        let ext_size = self.header.nsymbt as usize;
        if ext_size == 0 {
            return &[];
        }
        let end = 1024 + ext_size;
        &self.mmap[1024..end]
    }

    /// Get the raw data bytes from the memory map.
    ///
    /// This returns a slice starting at the beginning of voxel data
    /// (after the header and extended header).
    pub fn data_bytes(&self) -> &[u8] {
        let data_size = self.header.data_size().unwrap_or(0);
        let end = self.data_offset + data_size;
        if end > self.mmap.len() {
            return &self.mmap[self.data_offset..];
        }
        &self.mmap[self.data_offset..end]
    }

    /// Cross-check header statistics against actual data.
    ///
    /// Computes `dmin`, `dmax`, `dmean` and `rms` from the memory-mapped data
    /// block and compares them with the header values using a 1 % relative
    /// tolerance (matching Python `mrcfile`'s `np.isclose(rtol=0.01)`).
    ///
    /// # Errors
    /// Returns [`Error::StatsMismatch`] if any statistic deviates by more than 1 %.
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        crate::engine::stats::validate_header_stats(&self.header, self.data_bytes())
    }

    /// Read a block of voxels as raw bytes from the mmap.
    pub fn read_block_bytes(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<&[u8], Error> {
        let data_len = self.mmap.len().saturating_sub(self.data_offset);
        let (start, end) = crate::io::reader_common::validate_block_read(
            self.shape, self.mode(), data_len, offset, shape,
        )?;
        Ok(&self.mmap[self.data_offset + start..self.data_offset + end])
    }

    /// Read and decode a block of voxels to the specified type.
    ///
    /// Returns an error if `T` does not match the file's voxel mode.
    pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(bytes)?;
        Ok(VoxelBlock { offset, shape, data })
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// # Errors
    /// Returns `Error::ModeMismatch` if `T` does not match the file mode.
    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        crate::io::reader_common::decode_block(bytes, self.mode(), self.endian)
    }

    /// Iterate over slices (Z axis).
    pub fn slices<T: Voxel>(&self) -> SliceIter<'_, T, Self> {
        SliceIter::new(self, self.shape)
    }

    /// Iterate over slabs (k slices at a time).
    pub fn slabs<T: Voxel>(&self, k: usize) -> SlabIter<'_, T, Self> {
        SlabIter::new(self, self.shape, k)
    }

    /// Iterate over arbitrary blocks.
    pub fn blocks<T: Voxel>(&self, block_shape: [usize; 3]) -> BlockIter<'_, T, Self> {
        BlockIter::new(self, self.shape, block_shape)
    }

    /// Iterate over slices, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slices_f32(&self) -> Result<crate::SliceIterF32<'_>, Error> {
        crate::io::reader_common::slices_f32(
            self.shape,
            self.mode(),
            self.endian,
            |offset, shape| self.read_block_bytes(offset, shape).map(std::borrow::Cow::Borrowed),
        )
    }

    /// Iterate over slabs, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slabs_f32(&self, k: usize) -> Result<crate::SliceIterF32<'_>, Error> {
        crate::io::reader_common::slabs_f32(
            self.shape,
            self.mode(),
            self.endian,
            k,
            |offset, shape| self.read_block_bytes(offset, shape).map(std::borrow::Cow::Borrowed),
        )
    }

    /// Iterate over slices, automatically converting Mode 6 (`Uint16`) to `u8`.
    ///
    /// Returns an error if the file is not Mode 6 or if any value exceeds 255.
    pub fn slices_u8(&self) -> Result<impl Iterator<Item = Result<VoxelBlock<u8>, Error>> + '_, Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        Ok((0..nz).map(move |z| {
            let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
            let u16_data = self.decode_block::<u16>(bytes)?;
            let u8_data = crate::engine::convert::convert_u16_slice_to_u8(&u16_data)?;
            Ok(VoxelBlock {
                offset: [0, 0, z],
                shape: [nx, ny, 1],
                data: u8_data,
            })
        }))
    }

    /// Iterate over slices for Mode 0 (8-bit) files with signed/unsigned interpretation.
    ///
    /// Mode 0 files are ambiguous: some software writes signed bytes, others unsigned.
    /// This method lets you explicitly choose the interpretation and returns `f32` values.
    pub fn slices_mode0(
        &self,
        interp: crate::mode::M0Interpretation,
    ) -> impl Iterator<Item = Result<VoxelBlock<f32>, Error>> + '_ {
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        (0..nz).map(move |z| {
            if self.mode() != Mode::Int8 {
                return Err(Error::ModeMismatch {
                    file_mode: self.mode(),
                    requested_mode: Mode::Int8,
                });
            }
            let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
            let data = crate::engine::convert::reinterpret_m0(bytes, interp);
            Ok(VoxelBlock {
                offset: [0, 0, z],
                shape: [nx, ny, 1],
                data,
            })
        })
    }

    /// Iterate over slabs for Mode 0 (8-bit) files with signed/unsigned interpretation.
    pub fn slabs_mode0(
        &self,
        k: usize,
        interp: crate::mode::M0Interpretation,
    ) -> impl Iterator<Item = Result<VoxelBlock<f32>, Error>> + '_ {
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        let mut z = 0usize;
        let mut error_returned = false;
        std::iter::from_fn(move || {
            if error_returned {
                return None;
            }
            if self.mode() != Mode::Int8 {
                error_returned = true;
                return Some(Err(Error::ModeMismatch {
                    file_mode: self.mode(),
                    requested_mode: Mode::Int8,
                }));
            }
            if z >= nz {
                return None;
            }
            let start = z;
            let size = k.min(nz - z);
            z += size;
            let bytes = match self.read_block_bytes([0, 0, start], [nx, ny, size]) {
                Ok(b) => b,
                Err(e) => return Some(Err(e)),
            };
            let data = crate::engine::convert::reinterpret_m0(bytes, interp);
            Some(Ok(VoxelBlock {
                offset: [0, 0, start],
                shape: [nx, ny, size],
                data,
            }))
        })
    }
}


