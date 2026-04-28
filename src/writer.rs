//! MRC file writer with block-based API

use std::path::{Path, PathBuf};

use crate::engine::block::{SliceAccess, VolumeShape, VoxelBlock};
use crate::engine::codec::{encode_block_parallel, encode_slice};
use crate::engine::convert::Convert;
use crate::engine::endian::FileEndian;
use crate::{Error, Header, Mode};

use alloc::vec::Vec;

pub struct WriterBuilder {
    path: PathBuf,
    header: Header,
}

impl WriterBuilder {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: PathBuf::from(path.as_ref()),
            header: Header::new(),
        }
    }

    pub fn shape(mut self, shape: [usize; 3]) -> Self {
        self.header.nx = shape[0] as i32;
        self.header.ny = shape[1] as i32;
        self.header.nz = shape[2] as i32;
        self
    }

    pub fn mode<T: crate::mode::Voxel>(mut self) -> Self {
        self.header.mode = T::MODE as i32;
        self
    }

    pub fn finish(self) -> Result<Writer, Error> {
        Writer::create(&self.path, self.header)
    }
}

pub struct Writer {
    file: std::fs::File,
    header: Header,
    data_offset: u64,
    bytes_per_voxel: usize,
    shape: VolumeShape,
    data: Vec<u8>,
    #[cfg(feature = "parallel")]
    parallel_writes: bool,
}

impl Writer {
    #[cfg(feature = "std")]
    fn create(path: &Path, header: Header) -> Result<Self, Error> {
        use std::io::Write;

        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let mut file = std::fs::File::create(path).map_err(|_| Error::Io("create file".into()))?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        file.write_all(&header_bytes).map_err(|_| Error::Io("write header".into()))?;

        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            let zeros = alloc::vec![0u8; ext_size];
            file.write_all(&zeros).map_err(|_| Error::Io("write extended header".into()))?;
        }

        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let bytes_per_voxel = mode.byte_size();

        let data_size = header.data_size();
        let data = alloc::vec![0u8; data_size];

        let shape =
            VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            file,
            header,
            data_offset,
            bytes_per_voxel,
            shape,
            data,
            #[cfg(feature = "parallel")]
            parallel_writes: false,
        })
    }

    #[cfg(not(feature = "std"))]
    fn create(path: &Path, header: Header) -> Result<Self, Error> {
        let _ = path;
        let _ = header;
        Err(Error::Io("std feature not enabled".into()))
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Write a block of voxels to the file.
    ///
    /// This is the unified encode pipeline that handles:
    /// - Typed values → Endian encoding → Raw bytes
    pub fn write_block<T: crate::engine::codec::EndianCodec + Sync>(
        &mut self,
        block: &VoxelBlock<T>,
    ) -> Result<(), Error> {
        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        let start_offset = (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        let end_offset = start_offset + sx * sy * sz * self.bytes_per_voxel;

        if end_offset > self.data.len() {
            return Err(Error::BoundsError);
        }

        // Encode slice to bytes
        encode_slice(
            &block.data,
            &mut self.data[start_offset..end_offset],
            FileEndian::LittleEndian,
        );

        Ok(())
    }

    /// Write a block with type conversion.
    ///
    /// This method accepts data in one type (S) and converts it to the file's
    /// native voxel mode before writing.
    ///
    /// # Example
    /// ```ignore
    /// // Write Float32 data to an Int16 file
    /// let writer = Writer::create("output.mrc", header)?;
    /// let block: VoxelBlock<f32> = ...;
    /// writer.write_converted::<f32, i16>(&block)?; // Converts f32 -> i16
    /// ```
    pub fn write_converted<S, D>(&mut self, block: &VoxelBlock<S>) -> Result<(), Error>
    where
        S: crate::engine::codec::EndianCodec + Copy + 'static,
        D: Convert<S> + crate::engine::codec::EndianCodec + Copy + Default + crate::mode::Voxel + 'static,
    {
        // Try SIMD batch conversion first
        #[cfg(feature = "simd")]
        {
            use crate::engine::convert::try_simd_convert_reverse;
            if let Some(converted_data) = try_simd_convert_reverse::<S, D>(&block.data) {
                let converted_block = VoxelBlock {
                    offset: block.offset,
                    shape: block.shape,
                    data: converted_data,
                };
                return self.write_block::<D>(&converted_block);
            }
        }

        // Fall back to scalar conversion
        let converted_data: Vec<D> = block
            .data
            .iter()
            .map(|&src| D::convert(src))
            .collect();

        // Create a new block with converted data
        let converted_block = VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data: converted_data,
        };

        // Write using the standard path
        self.write_block::<D>(&converted_block)
    }
}

impl SliceAccess for Writer {
    fn slice_mut<T: crate::engine::codec::EndianCodec>(
        &mut self,
        z: usize,
    ) -> Result<&mut [T], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        if z >= nz {
            return Err(Error::BoundsError);
        }

        let start_offset = z * nx * ny * self.bytes_per_voxel;
        let end_offset = start_offset + nx * ny * self.bytes_per_voxel;

        let bytes = &mut self.data[start_offset..end_offset];
        unsafe {
            let ptr = bytes.as_mut_ptr() as *mut T;
            Ok(core::slice::from_raw_parts_mut(ptr, nx * ny))
        }
    }
}

impl Writer {
    /// Write a block with parallel encoding and file I/O
    #[cfg(all(feature = "parallel", feature = "std"))]
    pub fn write_block_parallel<T: crate::engine::codec::EndianCodec + Sync + Clone>(
        &mut self,
        block: &VoxelBlock<T>,
    ) -> Result<(), Error> {
        use rayon::prelude::*;
        use std::os::unix::fs::FileExt;

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let base_offset = ((ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel) as u64;

        // Mark that parallel writes were used
        self.parallel_writes = true;

        // Encode in parallel and write to file
        let encoded_chunks =
            encode_block_parallel(&block.data, chunk_size, FileEndian::LittleEndian);

        // Write chunks in parallel using pwrite
        encoded_chunks.par_iter().try_for_each(|(chunk_idx, encoded)| {
            let offset = base_offset + (*chunk_idx * chunk_size * self.bytes_per_voxel) as u64;
            self.file
                .write_all_at(encoded, offset)
                .map_err(|_| Error::Io("parallel write chunk".into()))
        })?;

        Ok(())
    }
    pub fn finalize(&mut self) -> Result<(), Error> {
        use std::io::{Seek, SeekFrom, Write};

        // Write all data to file
        #[cfg(feature = "parallel")]
        {
            if !self.parallel_writes {
                // Only write buffered data if parallel writes weren't used
                self.file
                    .seek(SeekFrom::Start(self.data_offset))
                    .map_err(|_| Error::Io("seek to data offset".into()))?;
                self.file.write_all(&self.data).map_err(|_| Error::Io("write voxel data".into()))?;
            }
        }

        #[cfg(not(feature = "parallel"))]
        {
            self.file
                .seek(SeekFrom::Start(self.data_offset))
                .map_err(|_| Error::Io("seek to data offset".into()))?;
            self.file.write_all(&self.data).map_err(|_| Error::Io("write voxel data".into()))?;
        }

        // Rewrite header
        self.file.seek(SeekFrom::Start(0)).map_err(|_| Error::Io("seek to header".into()))?;

        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        self.file.write_all(&header_bytes).map_err(|_| Error::Io("write header".into()))?;

        Ok(())
    }
}

#[cfg(feature = "mmap")]
pub struct MmapWriter {
    mmap: memmap2::MmapMut,
    header: Header,
    data_offset: usize,
    bytes_per_voxel: usize,
    shape: VolumeShape,
    #[cfg(feature = "parallel")]
    parallel_writes: bool,
}

#[cfg(feature = "mmap")]
impl MmapWriter {
    pub fn create(path: impl AsRef<Path>, header: Header) -> Result<Self, Error> {
        use std::fs::OpenOptions;
        use std::io::Write;

        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let total_size = header.data_offset() + header.data_size();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|_| Error::Io("create file for mmap".into()))?;

        file.set_len(total_size as u64).map_err(|_| Error::Io("set file length".into()))?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        file.write_all(&header_bytes).map_err(|_| Error::Io("write header".into()))?;

        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .map_mut(&file)
                .map_err(|_| Error::Mmap)?
        };

        let data_offset = header.data_offset();
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let bytes_per_voxel = mode.byte_size();

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            mmap,
            header,
            data_offset,
            bytes_per_voxel,
            shape,
            #[cfg(feature = "parallel")]
            parallel_writes: false,
        })
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    pub fn write_block<T: crate::engine::codec::EndianCodec + Sync>(
        &mut self,
        block: &VoxelBlock<T>,
    ) -> Result<(), Error> {
        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        let start_offset = self.data_offset + (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        let end_offset = start_offset + sx * sy * sz * self.bytes_per_voxel;

        if end_offset > self.mmap.len() {
            return Err(Error::BoundsError);
        }

        // Encode slice directly to mmap
        encode_slice(
            &block.data,
            &mut self.mmap[start_offset..end_offset],
            FileEndian::LittleEndian,
        );
        Ok(())
    }

    /// Write a block with parallel encoding to memory-mapped region
    #[cfg(all(feature = "parallel", feature = "std"))]
    pub fn write_block_parallel<T: crate::engine::codec::EndianCodec + Sync>(
        &mut self,
        block: &VoxelBlock<T>,
    ) -> Result<(), Error> {
        use rayon::prelude::*;

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let base_offset = self.data_offset + (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;

        // Mark that parallel writes were used
        self.parallel_writes = true;

        // Get raw pointer as usize for parallel writes
        let mmap_ptr = self.mmap.as_mut_ptr() as usize;

        // Encode and write to mmap in parallel
        block
            .data
            .par_chunks(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start_offset = base_offset + chunk_idx * chunk_size * self.bytes_per_voxel;
                let ptr = (mmap_ptr + start_offset) as *mut u8;
                let dst = unsafe {
                    core::slice::from_raw_parts_mut(ptr, chunk.len() * self.bytes_per_voxel)
                };

                // Encode chunk directly to mmap
                encode_slice(chunk, dst, FileEndian::LittleEndian);
            });

        Ok(())
    }
}

#[cfg(feature = "mmap")]
impl SliceAccess for MmapWriter {
    fn slice_mut<T: crate::engine::codec::EndianCodec>(
        &mut self,
        z: usize,
    ) -> Result<&mut [T], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        if z >= nz {
            return Err(Error::BoundsError);
        }

        let start_offset = self.data_offset + z * nx * ny * self.bytes_per_voxel;
        let end_offset = start_offset + nx * ny * self.bytes_per_voxel;

        let bytes = &mut self.mmap[start_offset..end_offset];
        unsafe {
            let ptr = bytes.as_mut_ptr() as *mut T;
            Ok(core::slice::from_raw_parts_mut(ptr, nx * ny))
        }
    }
}

#[cfg(feature = "mmap")]
impl MmapWriter {
    pub fn finalize(&mut self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        self.mmap[0..1024].copy_from_slice(&header_bytes);
        self.mmap.flush().map_err(|_| Error::Io("flush mmap".into()))?;
        Ok(())
    }
}
