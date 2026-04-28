//! Memory-mapped MRC file reader with zero-copy API

#[cfg(feature = "mmap")]
use std::path::Path;

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::{EndianCodec, decode_slice};
use crate::engine::convert::Convert;
use crate::engine::endian::FileEndian;
use crate::engine::pipeline::{ConversionPath, get_conversion_path, is_zero_copy};
use crate::{Error, Header, Mode};

use alloc::vec::Vec;

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
/// // Iterate over slices with zero-copy when possible
/// for slice in reader.slices::<f32>() {
///     let block = slice?;
///     // process block.data
/// }
/// ```
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
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path).map_err(|_| Error::Io("open file for mmap".into()))?;

        // Read header first (not mapped, since we need to parse it)
        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)
            .map_err(|_| Error::Io("read header".into()))?;

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

    /// Get the raw data bytes from the memory map.
    ///
    /// This returns a slice starting at the beginning of voxel data
    /// (after the header and extended header).
    pub fn data_bytes(&self) -> &[u8] {
        let data_size = self.header.data_size();
        &self.mmap[self.data_offset..self.data_offset + data_size]
    }

    /// Read a block of voxels as raw bytes from the mmap.
    fn read_voxel_bytes(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<&[u8], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let start_byte = self.data_offset
            + (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        let end_byte = start_byte + sx * sy * sz * self.bytes_per_voxel;

        if end_byte > self.mmap.len() {
            return Err(Error::BoundsError);
        }

        Ok(&self.mmap[start_byte..end_byte])
    }

    /// Check if zero-copy decode is possible.
    ///
    /// Zero-copy requires: file mode matches T's mode AND file endian is native.
    pub fn can_zero_copy<T: crate::mode::Voxel>(&self) -> bool {
        is_zero_copy(self.mode(), T::MODE, self.endian)
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// Uses zero-copy fast path when mode and endian match.
    pub(crate) fn decode_block<T: EndianCodec + Send + Copy + Default + crate::mode::Voxel>(
        &self,
        bytes: &[u8],
    ) -> Result<Vec<T>, Error> {
        if self.can_zero_copy::<T>() {
            return self.decode_block_zero_copy(bytes);
        }

        Ok(decode_slice(bytes, self.endian))
    }

    /// Zero-copy decode: transmute bytes directly to Vec<T>.
    fn decode_block_zero_copy<T: EndianCodec + Copy>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
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

    /// Decode and convert bytes to destination type.
    pub(crate) fn decode_and_convert<S, D>(&self, bytes: &[u8]) -> Result<Vec<D>, Error>
    where
        S: EndianCodec + Send + Copy + Default + 'static,
        D: Convert<S> + 'static,
    {
        let src_data = decode_slice::<S>(bytes, self.endian);

        #[cfg(feature = "simd")]
        {
            use crate::engine::convert::try_simd_convert;
            if let Some(result) = try_simd_convert::<S, D>(&src_data) {
                return Ok(result);
            }
        }

        let mut dst_data = Vec::with_capacity(src_data.len());
        for src in src_data {
            dst_data.push(D::convert(src));
        }

        Ok(dst_data)
    }

    /// Read and convert voxels from file mode to target type D.
    pub fn read_converted<S, D>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<VoxelBlock<D>, Error>
    where
        S: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
        D: Convert<S> + EndianCodec + Copy + Default + crate::mode::Voxel,
    {
        let bytes = self.read_voxel_bytes(offset, shape)?;
        let data = self.decode_and_convert::<S, D>(bytes)?;
        Ok(VoxelBlock { offset, shape, data })
    }

    /// Get the optimal conversion path.
    pub fn conversion_path(&self, dst_mode: Mode) -> ConversionPath {
        get_conversion_path(self.mode(), dst_mode, self.endian)
    }

    /// Iterate over slices (Z axis).
    pub fn slices<T>(&self) -> MmapSliceIter<'_, T> {
        MmapSliceIter::new(self, self.shape)
    }

    /// Iterate over slabs (k slices at a time).
    pub fn slabs<T>(&self, k: usize) -> MmapSlabIter<'_, T> {
        MmapSlabIter::new(self, self.shape, k)
    }

    /// Iterate over arbitrary blocks.
    pub fn blocks<T>(&self, block_shape: [usize; 3]) -> MmapBlockIter<'_, T> {
        MmapBlockIter::new(self, self.shape, block_shape)
    }

    /// Iterate over slices with type conversion.
    pub fn slices_converted<S, D>(&self) -> MmapSliceIterConverted<'_, S, D>
    where
        S: crate::mode::Voxel,
        D: Convert<S> + crate::mode::Voxel,
    {
        MmapSliceIterConverted::new(self, self.shape)
    }

    /// Iterate over slabs with type conversion.
    pub fn slabs_converted<S, D>(&self, k: usize) -> MmapSlabIterConverted<'_, S, D>
    where
        S: crate::mode::Voxel,
        D: Convert<S> + crate::mode::Voxel,
    {
        MmapSlabIterConverted::new(self, self.shape, k)
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
    T: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
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

        match self.reader.read_voxel_bytes(offset, shape) {
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
        self.z = n;
        self.next()
    }
}

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
            slab_size: k,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for MmapSlabIter<'a, T>
where
    T: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
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

        match self.reader.read_voxel_bytes(offset, shape) {
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
        self.z = n * self.slab_size;
        self.next()
    }
}

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
    T: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
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

        match self.reader.read_voxel_bytes(offset, shape) {
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

// ============================================================================
// Conversion-enabled Iterators for MmapReader
// ============================================================================

/// Slice iterator with type conversion for MmapReader.
pub struct MmapSliceIterConverted<'a, S, D> {
    reader: &'a MmapReader,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    _phantom: core::marker::PhantomData<(S, D)>,
}

impl<'a, S, D> MmapSliceIterConverted<'a, S, D> {
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

impl<'a, S, D> Iterator for MmapSliceIterConverted<'a, S, D>
where
    S: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
    D: Convert<S> + EndianCodec + Copy + Default + crate::mode::Voxel,
{
    type Item = Result<VoxelBlock<D>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let z = self.z;
        self.z += 1;

        Some(self.reader.read_converted::<S, D>([0, 0, z], [self.nx, self.ny, 1]))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = n;
        self.next()
    }
}

/// Slab iterator with type conversion for MmapReader.
pub struct MmapSlabIterConverted<'a, S, D> {
    reader: &'a MmapReader,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    slab_size: usize,
    _phantom: core::marker::PhantomData<(S, D)>,
}

impl<'a, S, D> MmapSlabIterConverted<'a, S, D> {
    pub fn new(reader: &'a MmapReader, shape: VolumeShape, k: usize) -> Self {
        Self {
            reader,
            z: 0,
            nz: shape.nz,
            nx: shape.nx,
            ny: shape.ny,
            slab_size: k,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, S, D> Iterator for MmapSlabIterConverted<'a, S, D>
where
    S: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
    D: Convert<S> + EndianCodec + Copy + Default + crate::mode::Voxel,
{
    type Item = Result<VoxelBlock<D>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let z = self.z;
        let size = self.slab_size.min(self.nz - z);
        self.z += size;

        Some(self.reader.read_converted::<S, D>([0, 0, z], [self.nx, self.ny, size]))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = n * self.slab_size;
        self.next()
    }
}
