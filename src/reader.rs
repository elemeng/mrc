//! MRC file reader with iterator-centric API

use crate::engine::block::VolumeShape;
use crate::engine::codec::{EndianCodec, decode_slice};
use crate::engine::convert::Convert;
use crate::engine::endian::FileEndian;
use crate::engine::pipeline::{ConversionPath, get_conversion_path, is_zero_copy};
use crate::iter::{BlockIter, SliceIter, SliceIterConverted, SlabIter, SlabIterConverted};
use crate::{Error, Header, Mode};

use std::vec::Vec;

pub struct Reader {
    header: Header,
    ext_header: Vec<u8>,
    data: Vec<u8>,
    endian: FileEndian,
    shape: VolumeShape,
}

impl Reader {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;

        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let header = Header::decode_from_bytes(&header_bytes);

        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let data_size = header.data_size();

        let ext_size = header.nsymbt as usize;
        let mut ext_header = vec![0u8; ext_size];
        if ext_size > 0 {
            file.read_exact(&mut ext_header)?;
        }

        let mut data = vec![0u8; data_size];
        file.read_exact(&mut data)?;

        let endian = header.detect_endian();
        let shape =
            VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            header,
            ext_header,
            data,
            endian,
            shape,
        })
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

    pub fn slices<T>(&self) -> SliceIter<'_, T> {
        SliceIter::new(self, self.shape)
    }

    pub fn slabs<T>(&self, k: usize) -> SlabIter<'_, T> {
        SlabIter::new(self, self.shape, k)
    }

    pub fn blocks<T>(&self, chunk_shape: [usize; 3]) -> BlockIter<'_, T> {
        BlockIter::new(self, self.shape, chunk_shape)
    }

    /// Read a block of raw voxel bytes from the file.
    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let bytes_per_voxel = self.mode().byte_size();

        let start_byte = (ox + oy * nx + oz * nx * ny) * bytes_per_voxel;
        let end_byte = start_byte + sx * sy * sz * bytes_per_voxel;

        if end_byte > self.data.len() {
            return Err(Error::BoundsError);
        }

        Ok(self.data[start_byte..end_byte].to_vec())
    }

    /// Read and decode a block of voxels to the specified type.
    pub fn read_block<T: EndianCodec + Send + Copy + Default + crate::mode::Voxel>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<crate::engine::block::VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(crate::engine::block::VoxelBlock { offset, shape, data })
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// This is the unified decode pipeline that handles:
    /// - Layer 1 → Layer 2: Raw bytes → Endian normalization
    /// - Layer 2 → Layer 3: Endian-normalized → Typed values
    ///
    /// Uses zero-copy fast path when:
    /// - src_mode == dst_mode (T matches file mode)
    /// - file_endian == native
    pub(crate) fn decode_block<T: EndianCodec + Send + Copy + Default + crate::mode::Voxel>(
        &self,
        bytes: &[u8],
    ) -> Result<Vec<T>, Error> {
        // Zero-copy fast path: when mode matches and endian is native,
        // we can directly transmute the bytes to the target type
        if self.can_zero_copy::<T>() {
            return self.decode_block_zero_copy(bytes);
        }

        // Standard decode path with safe initialization
        Ok(decode_slice(bytes, self.endian))
    }

    /// Zero-copy decode: transmute bytes directly to Vec<T>
    /// 
    /// # Safety
    /// This is only safe when:
    /// - The file mode matches T's mode exactly
    /// - The file endian matches native endian
    fn decode_block_zero_copy<T: EndianCodec + Copy>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        let n = bytes.len() / T::BYTE_SIZE;
        
        // SAFETY: We've verified mode and endian match, so we can reinterpret bytes as T
        // Allocate uninitialized memory and copy bytes directly
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

    /// Check if zero-copy decode is possible for the given type.
    /// Zero-copy requires: file mode matches T's mode AND file endian is native.
    pub fn can_zero_copy<T: crate::mode::Voxel>(&self) -> bool {
        is_zero_copy(self.mode(), T::MODE, self.endian)
    }

    /// Read and convert voxels from file mode to target type D.
    ///
    /// This method decodes the raw bytes using the file's voxel mode,
    /// then converts each voxel to the destination type D using the
    /// Convert trait.
    ///
    /// # Example
    /// ```ignore
    /// // Read Int16 file as Float32
    /// let reader = Reader::open("data.mrc")?;
    /// let block: VoxelBlock<f32> = reader.read_block_converted::<i16, f32>([0,0,0], [64,64,64])?;
    /// ```
    pub fn read_block_converted<S, D>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<crate::engine::block::VoxelBlock<D>, Error>
    where
        S: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
        D: Convert<S> + EndianCodec + Copy + Default + crate::mode::Voxel,
    {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_and_convert::<S, D>(&bytes)?;
        Ok(crate::engine::block::VoxelBlock { offset, shape, data })
    }

    /// Deprecated alias for `read_block_converted`.
    #[deprecated(since = "0.2.4", note = "Use `read_block_converted` instead")]
    pub fn read_converted<S, D>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<crate::engine::block::VoxelBlock<D>, Error>
    where
        S: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
        D: Convert<S> + EndianCodec + Copy + Default + crate::mode::Voxel,
    {
        self.read_block_converted::<S, D>(offset, shape)
    }

    /// Read and convert voxels with checked conversion, failing on out-of-range values.
    pub fn read_block_checked_converted<S, D>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<crate::engine::block::VoxelBlock<D>, Error>
    where
        S: EndianCodec + Send + Copy + Default + crate::mode::Voxel,
        D: crate::engine::convert::CheckedConvert<S> + EndianCodec + Copy + Default + crate::mode::Voxel,
    {
        let bytes = self.read_block_bytes(offset, shape)?;
        let src_data = decode_slice::<S>(&bytes, self.endian);
        let mut dst_data = Vec::with_capacity(src_data.len());
        for src in src_data {
            dst_data.push(D::try_convert(src) .map_err(Error::Conversion)?);
        }
        Ok(crate::engine::block::VoxelBlock { offset, shape, data: dst_data })
    }

    /// Decode and convert bytes to destination type D through intermediate type S.
    ///
    /// Pipeline: bytes → decode<S>() → convert → Vec<D>
    /// Uses SIMD batch conversions when available and applicable.
    pub(crate) fn decode_and_convert<S, D>(&self, bytes: &[u8]) -> Result<Vec<D>, Error>
    where
        S: EndianCodec + Send + Copy + Default + 'static,
        D: Convert<S> + 'static,
    {
        // First decode to source type
        let src_data = decode_slice::<S>(bytes, self.endian);

        // Try SIMD batch conversion for common f32 destinations
        #[cfg(feature = "simd")]
        {
            use crate::engine::convert::try_simd_convert;
            if let Some(result) = try_simd_convert::<S, D>(&src_data) {
                return Ok(result);
            }
        }

        // Fall back to scalar conversion
        let mut dst_data = Vec::with_capacity(src_data.len());
        for src in src_data {
            dst_data.push(D::convert(src));
        }

        Ok(dst_data)
    }

    /// Get the optimal conversion path for the given source and destination modes.
    pub fn conversion_path(&self, dst_mode: Mode) -> ConversionPath {
        get_conversion_path(self.mode(), dst_mode, self.endian)
    }

    /// Iterate over slices with type conversion.
    ///
    /// # Example
    /// ```ignore
    /// // Read Int16 file as Float32 slices
    /// let reader = Reader::open("data.mrc")?;
    /// for slice in reader.slices_converted::<i16, f32>() {
    ///     let data: Vec<f32> = slice?.data; // converted from i16
    /// }
    /// ```
    pub fn slices_converted<S, D>(&self) -> SliceIterConverted<'_, S, D>
    where
        S: crate::mode::Voxel,
        D: Convert<S> + crate::mode::Voxel,
    {
        SliceIterConverted::new(self, self.shape)
    }

    /// Iterate over slabs with type conversion.
    ///
    /// # Example
    /// ```ignore
    /// // Read Int16 file as Float32 slabs of 10 slices each
    /// let reader = Reader::open("data.mrc")?;
    /// for slab in reader.slabs_converted::<i16, f32>(10) {
    ///     let data: Vec<f32> = slab?.data; // converted from i16
    /// }
    /// ```
    pub fn slabs_converted<S, D>(&self, k: usize) -> SlabIterConverted<'_, S, D>
    where
        S: crate::mode::Voxel,
        D: Convert<S> + crate::mode::Voxel,
    {
        SlabIterConverted::new(self, self.shape, k)
    }

    /// Iterate over slices for Mode 0 (8-bit) files with signed/unsigned interpretation.
    ///
    /// Mode 0 files are ambiguous: some software writes signed bytes, others unsigned.
    /// This method lets you explicitly choose the interpretation and returns `f32` values.
    pub fn slices_mode0(
        &self,
        interp: crate::mode::M0Interpretation,
    ) -> impl Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + '_ {
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        (0..nz).map(move |z| {
            let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
            let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
            Ok(crate::engine::block::VoxelBlock {
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
    ) -> impl Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + '_ {
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
            let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
            Some(Ok(crate::engine::block::VoxelBlock {
                offset: [0, 0, start],
                shape: [nx, ny, size],
                data,
            }))
        })
    }
}

impl Reader {
    /// Get a reference to the raw data bytes
    pub fn data(&self) -> &[u8] {
        &self.data
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
