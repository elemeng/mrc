//! Memory-mapped MRC file reader with zero-copy API.
//!
//! Provides [`MmapReader`], which maps the file into the process address space
//! rather than copying it into a `Vec<u8>`. The OS pages data in and out on
//! demand, making this ideal for very large files or when only a small subset
//! of slices needs to be accessed.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::vec::Vec;

/// Memory-mapped MRC file reader.
///
/// Maps the file into the process address space rather than loading it
/// into a heap-allocated buffer. The OS pages data in and out on demand,
/// making this ideal for very large files or when only a small subset of
/// slices needs to be accessed.
///
/// # Zero-copy access
///
/// [`slab_as`](Self::slab_as) returns `&[T]` directly into the memory map
/// with no allocation — this is true zero-copy.  It requires the file
/// endianness to match the host and `T` to match the voxel mode.
///
/// [`data_bytes`](Self::data_bytes) returns a `&[u8]` view of the raw
/// voxel data, also zero-copy.
///
/// For non-native-endian files or type-mismatched reads, use
/// [`read_block`](Self::read_block) which always allocates.
///
/// # Example
/// ```no_run
/// use mrc::MmapReader;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let reader = MmapReader::open("large_file.mrc")?;
///     println!("Dimensions: {:?}", reader.shape());
///
///     // Zero-copy typed access (native endian, mode-matching type)
///     let slice: &[f32] = reader.slab_as::<f32>(0, 1)?;
///     println!("First slice has {} voxels", slice.len());
///
///     // Generic typed iteration (always allocates per block)
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

        // Read header first (not mapped, since we need to parse it)
        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let (header, warnings, endian, data_size) =
            crate::io::reader_common::parse_header(&header_bytes, permissive)?;

        // Map the entire file
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&file)
                .map_err(|_| Error::Mmap)?
        };

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        // Validate file size
        let expected_size = header
            .data_offset()
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
        Ok((
            Self {
                mmap,
                header,
                data_offset: header.data_offset(),
                endian,
                shape,
            },
            warnings,
        ))
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

    /// Zero-copy read of a contiguous Z-slab as `&[T]`.
    ///
    /// Returns a slice pointing directly into the memory map, avoiding
    /// any allocation or copying. This is the most efficient way to access
    /// voxel data when the file endianness matches the host.
    ///
    /// # Requirements
    ///
    /// * `T::MODE` must match the file's voxel mode
    /// * File endianness must be native (host byte order)
    /// * `k > 0` and `z + k <= nz`
    ///
    /// For non-native-endian files or type mismatches, use
    /// [`read_block`](Self::read_block) instead.
    pub fn slab_as<T: Voxel>(&self, z: usize, k: usize) -> Result<&[T], Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }
        if !self.endian.is_native() {
            return Err(Error::TypeMismatch {
                expected: T::BYTE_SIZE,
                actual: T::BYTE_SIZE,
            });
        }

        let b = T::BYTE_SIZE;
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];

        if k == 0 || z + k > nz {
            return Err(Error::BoundsError);
        }

        let linear_start = z * nx * ny;
        let byte_start = self.data_offset + linear_start * b;
        let count = nx * ny * k;
        let byte_end = byte_start + count * b;

        if byte_end > self.mmap.len() {
            return Err(Error::BoundsError);
        }

        // SAFETY:
        // • T::MODE matches the file mode (checked above), so the on-disk
        //   byte layout is exactly `T`.
        // • Native endian (checked above), so byte order matches the host.
        // • `data_offset` (= 1024 + nsymbt) is 4-byte aligned for any
        //   valid `nsymbt`.  All MRC voxel types have sizes 1, 2, 4, or 8,
        //   so their alignment is always satisfied by a 4-byte-aligned base.
        // • Bounds are checked above, so the byte range falls within the mmap.
        // • `T: Copy` (from the `Voxel` bound) makes it safe to read the
        //   same bytes from multiple references without aliasing concerns.
        unsafe {
            let ptr = self.mmap.as_ptr().add(byte_start) as *const T;
            Ok(core::slice::from_raw_parts(ptr, count))
        }
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
    ///
    /// Supports arbitrary sub-blocks; non-contiguous regions are gathered
    /// into a contiguous buffer automatically.
    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        self.read_block_bytes_cow(offset, shape)
            .map(|c| c.into_owned())
    }

    /// Like [`read_block_bytes`](Self::read_block_bytes) but returns
    /// `Cow::Borrowed` for contiguous XY slabs (zero-copy fast path).
    pub(crate) fn read_block_bytes_cow<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<std::borrow::Cow<'a, [u8]>, Error> {
        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;
        let b = self.mode().byte_size();

        let data_len = self.mmap.len().saturating_sub(self.data_offset);
        crate::io::reader_common::validate_block_bounds(
            self.shape,
            self.mode(),
            data_len,
            offset,
            shape,
        )?;

        // Fast path: full XY slab is contiguous — borrow directly from the mmap.
        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear = oz * nx * ny;
            let start = self.data_offset + linear * b;
            let byte_len = sx * sy * sz * b;
            return Ok(std::borrow::Cow::Borrowed(
                &self.mmap[start..start + byte_len],
            ));
        }

        // Non-contiguous block: gather into owned Vec.
        Ok(std::borrow::Cow::Owned(
            crate::io::reader_common::gather_block_bytes(
                &self.mmap[self.data_offset..],
                self.shape,
                self.mode(),
                offset,
                shape,
            ),
        ))
    }

    /// Read and decode a block of voxels to the specified type.
    ///
    /// Returns an error if `T` does not match the file's voxel mode.
    pub fn read_block<T: Voxel>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(VoxelBlock {
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
