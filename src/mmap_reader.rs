//! Memory-mapped MRC file reader with zero-copy API

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::{EndianCodec, decode_slice};
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::vec::Vec;

/// Memory-mapped MRC file reader.
///
/// Provides zero-copy access to MRC files by memory-mapping them into the process address space.
/// This is ideal for reading large files that don't fit in RAM, as the OS handles paging.
///
/// # Example
/// ```ignore
/// use mrc::MmapReader;
///
/// let reader = MmapReader::open("large_file.mrc")?;
/// println!("Dimensions: {:?}", reader.shape());
///
/// // Iterate over slices with zero-copy when file type matches
/// for slice in reader.slices::<f32>() {
///     let block = slice?;
///     // process block.data
/// }
/// ```
/// Iterator over slices yielding `f32` voxel blocks.
#[cfg(feature = "mmap")]
pub type MmapSliceIterF32<'a> = Box<dyn Iterator<Item = Result<VoxelBlock<f32>, Error>> + 'a>;

#[cfg(feature = "mmap")]
pub struct MmapReader {
    mmap: memmap2::Mmap,
    header: Header,
    data_offset: usize,
    endian: FileEndian,
    shape: VolumeShape,
    bytes_per_voxel: usize,
}

#[cfg(feature = "mmap")]
impl MmapReader {
    /// Open an MRC file via memory mapping.
    ///
    /// The file is mapped read-only into the process address space.
    /// The OS will page data in/out as needed, making this efficient for large files.
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;

        // Read header first (not mapped, since we need to parse it)
        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let header = Header::decode_from_bytes(&header_bytes);

        if !header.validate() {
            return Err(Error::InvalidHeader);
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

        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let bytes_per_voxel = mode.byte_size();

        Ok(Self {
            mmap,
            header,
            data_offset: header.data_offset(),
            endian,
            shape,
            bytes_per_voxel,
        })
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
    pub fn ext_header_bytes(&self) -> Result<&[u8], Error> {
        let ext_size = self.header.nsymbt as usize;
        if ext_size == 0 {
            return Ok(&[]);
        }
        let end = 1024 + ext_size;
        if end > self.mmap.len() {
            return Err(Error::InvalidHeader);
        }
        Ok(&self.mmap[1024..end])
    }

    /// Get the raw data bytes from the memory map.
    ///
    /// This returns a slice starting at the beginning of voxel data
    /// (after the header and extended header).
    pub fn data_bytes(&self) -> Result<&[u8], Error> {
        let data_size = self.header.data_size();
        let end = self.data_offset + data_size;
        if end > self.mmap.len() {
            return Err(Error::InvalidHeader);
        }
        Ok(&self.mmap[self.data_offset..end])
    }

    /// Read a block of voxels as raw bytes from the mmap.
    pub fn read_block_bytes(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<&[u8], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        if self.mode() == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }

        let start_byte = self.data_offset
            + (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        let end_byte = start_byte + sx * sy * sz * self.bytes_per_voxel;

        if end_byte > self.mmap.len() {
            return Err(Error::BoundsError);
        }

        Ok(&self.mmap[start_byte..end_byte])
    }

    /// Read and decode a block of voxels to the specified type.
    ///
    /// Returns an error if `T` does not match the file's voxel mode.
    pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error> {
        if T::MODE == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(bytes)?;
        Ok(VoxelBlock { offset, shape, data })
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// # Errors
    /// Returns `Error::ModeMismatch` if `T` does not match the file mode.
    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if self.endian == FileEndian::native() {
            self.decode_block_native_endian(bytes)
        } else {
            Ok(decode_slice(bytes, self.endian))
        }
    }

    /// Native-endian decode: memcpy bytes directly to Vec<T>.
    fn decode_block_native_endian<T: EndianCodec + Copy>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        let n = bytes.len() / T::BYTE_SIZE;
        let mut result = Vec::with_capacity(n);
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                result.as_mut_ptr() as *mut u8,
                bytes.len(),
            );
            result.set_len(n);
        }
        Ok(result)
    }

    /// Iterate over slices (Z axis).
    pub fn slices<T: Voxel>(&self) -> MmapSliceIter<'_, T> {
        MmapSliceIter::new(self, self.shape)
    }

    /// Iterate over slabs (k slices at a time).
    pub fn slabs<T: Voxel>(&self, k: usize) -> MmapSlabIter<'_, T> {
        MmapSlabIter::new(self, self.shape, k)
    }

    /// Iterate over arbitrary blocks.
    pub fn blocks<T: Voxel>(&self, block_shape: [usize; 3]) -> MmapBlockIter<'_, T> {
        MmapBlockIter::new(self, self.shape, block_shape)
    }

    /// Iterate over slices, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slices_f32(&self) -> Result<MmapSliceIterF32<'_>, Error> {
        match self.mode() {
            Mode::Float32 => Ok(Box::new(self.slices::<f32>() )),
            Mode::Int16 => Ok(Box::new(self.slices::<i16>().map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Uint16 => Ok(Box::new(self.slices::<u16>().map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_u16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Int8 => Ok(Box::new(self.slices::<i8>().map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i8_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            _ => Err(Error::UnsupportedMode),
        }
    }

    /// Iterate over slabs, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slabs_f32(&self, k: usize) -> Result<MmapSliceIterF32<'_>, Error> {
        match self.mode() {
            Mode::Float32 => Ok(Box::new(self.slabs::<f32>(k) )),
            Mode::Int16 => Ok(Box::new(self.slabs::<i16>(k).map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Uint16 => Ok(Box::new(self.slabs::<u16>(k).map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_u16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Int8 => Ok(Box::new(self.slabs::<i8>(k).map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i8_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            _ => Err(Error::UnsupportedMode),
        }
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
        std::iter::from_fn(move || {
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

// ============================================================================
// MmapReader Iterators
// ============================================================================

/// Slice iterator for MmapReader.
pub struct MmapSliceIter<'a, T> {
    reader: &'a MmapReader,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> MmapSliceIter<'a, T> {
    pub fn new(reader: &'a MmapReader, shape: VolumeShape) -> Self {
        Self {
            reader,
            z: 0,
            nz: shape.nz,
            nx: shape.nx,
            ny: shape.ny,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for MmapSliceIter<'a, T>
where
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let z = self.z;
        self.z += 1;

        let offset = [0, 0, z];
        let shape = [self.nx, self.ny, 1];

        match self.reader.read_block_bytes(offset, shape) {
            Ok(bytes) => {
                match self.reader.decode_block::<T>(bytes) {
                    Ok(data) => Some(Ok(VoxelBlock { offset, shape, data })),
                    Err(e) => Some(Err(e)),
                }
            }
            Err(e) => Some(Err(e)),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = self.z.saturating_add(n);
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nz.saturating_sub(self.z);
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for MmapSliceIter<'a, T> where T: Voxel {}
impl<'a, T> core::iter::FusedIterator for MmapSliceIter<'a, T> where T: Voxel {}

/// Slab iterator for MmapReader.
pub struct MmapSlabIter<'a, T> {
    reader: &'a MmapReader,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    slab_size: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> MmapSlabIter<'a, T> {
    pub fn new(reader: &'a MmapReader, shape: VolumeShape, k: usize) -> Self {
        Self {
            reader,
            z: 0,
            nz: shape.nz,
            nx: shape.nx,
            ny: shape.ny,
            slab_size: k.max(1),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for MmapSlabIter<'a, T>
where
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let z = self.z;
        let size = self.slab_size.min(self.nz - z);
        self.z += size;

        let offset = [0, 0, z];
        let shape = [self.nx, self.ny, size];

        match self.reader.read_block_bytes(offset, shape) {
            Ok(bytes) => {
                match self.reader.decode_block::<T>(bytes) {
                    Ok(data) => Some(Ok(VoxelBlock { offset, shape, data })),
                    Err(e) => Some(Err(e)),
                }
            }
            Err(e) => Some(Err(e)),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = self.z.saturating_add(n * self.slab_size);
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nz.saturating_sub(self.z);
        let count = remaining.div_ceil(self.slab_size);
        (count, Some(count))
    }
}

impl<'a, T> ExactSizeIterator for MmapSlabIter<'a, T> where T: Voxel {}
impl<'a, T> core::iter::FusedIterator for MmapSlabIter<'a, T> where T: Voxel {}

/// Block iterator for MmapReader.
pub struct MmapBlockIter<'a, T> {
    reader: &'a MmapReader,
    position: [usize; 3],
    shape: VolumeShape,
    block_shape: [usize; 3],
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> MmapBlockIter<'a, T> {
    pub fn new(reader: &'a MmapReader, shape: VolumeShape, block_shape: [usize; 3]) -> Self {
        assert!(block_shape[0] > 0 && block_shape[1] > 0 && block_shape[2] > 0, "block_shape must be positive in all dimensions");
        Self {
            reader,
            position: [0, 0, 0],
            shape,
            block_shape,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for MmapBlockIter<'a, T>
where
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [bx, by, bz] = self.block_shape;
        let [px, py, pz] = self.position;

        if pz >= nz {
            return None;
        }

        let sx = bx.min(nx - px);
        let sy = by.min(ny - py);
        let sz = bz.min(nz - pz);

        // Update position for next iteration
        self.position[0] += bx;
        if self.position[0] >= nx {
            self.position[0] = 0;
            self.position[1] += by;
            if self.position[1] >= ny {
                self.position[1] = 0;
                self.position[2] += bz;
            }
        }

        let offset = [px, py, pz];
        let shape = [sx, sy, sz];

        match self.reader.read_block_bytes(offset, shape) {
            Ok(bytes) => {
                match self.reader.decode_block::<T>(bytes) {
                    Ok(data) => Some(Ok(VoxelBlock { offset, shape, data })),
                    Err(e) => Some(Err(e)),
                }
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl<'a, T> core::iter::FusedIterator for MmapBlockIter<'a, T> where T: Voxel {}
