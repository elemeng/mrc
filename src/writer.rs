//! MRC file writer with block-based API

use crate::{block::VoxelBlock, Error, FileEndian, Header, Mode, VolumeShape};
use crate::encode::Encode;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[cfg(feature = "mmap")]

pub struct WriterBuilder {
    path: String,
    header: Header,
}

impl WriterBuilder {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            header: Header::new(),
        }
    }

    pub fn shape(mut self, shape: [usize; 3]) -> Self {
        self.header.nx = shape[0] as i32;
        self.header.ny = shape[1] as i32;
        self.header.nz = shape[2] as i32;
        self
    }

    pub fn mode<T>(mut self) -> Self {
        let mode = match core::any::type_name::<T>() {
            "i8" => 0,
            "i16" => 1,
            "f32" => 2,
            "u16" => 6,
            "f16" => 12,
            _ => 2,
        };
        self.header.mode = mode;
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
    parallel_writes: bool,  // Track if parallel writes were used
}

impl Writer {
    fn create(path: &str, header: Header) -> Result<Self, Error> {
        #[cfg(feature = "std")]
        {
            use std::io::Write;

            if !header.validate() {
                return Err(Error::InvalidHeader);
            }

            let mut file = std::fs::File::create(path).map_err(|_| Error::Io)?;

            let mut header_bytes = [0u8; 1024];
            header.encode_to_bytes(&mut header_bytes);
            file.write_all(&header_bytes).map_err(|_| Error::Io)?;

            let ext_size = header.nsymbt as usize;
            if ext_size > 0 {
                let zeros = alloc::vec![0u8; ext_size];
                file.write_all(&zeros).map_err(|_| Error::Io)?;
            }

            let data_offset = header.data_offset() as u64;
            let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
            let bytes_per_voxel = match mode {
                Mode::Float32 => 4,
                Mode::Int16 => 2,
                Mode::Uint16 => 2,
                Mode::Int8 => 1,
                _ => return Err(Error::UnsupportedMode),
            };

            let data_size = header.data_size();
            let data = alloc::vec![0u8; data_size];

            let shape = VolumeShape::new(
                header.nx as usize,
                header.ny as usize,
                header.nz as usize,
            );

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
        {
            let _ = path;
            let _ = header;
            Err(Error::Io)
        }
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    pub fn write_block(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let start_offset = (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        let end_offset = start_offset + sx * sy * sz * self.bytes_per_voxel;

        if end_offset > self.data.len() {
            return Err(Error::BoundsError);
        }

        // Encode and write to buffer
        for (i, &val) in block.data.iter().enumerate() {
            val.encode(&mut self.data, start_offset + i * 4, FileEndian::LittleEndian);
        }

        Ok(())
    }

    pub fn slice_mut(&mut self, z: usize) -> Result<&mut [f32], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        if z >= nz {
            return Err(Error::BoundsError);
        }

        let start_offset = z * nx * ny * self.bytes_per_voxel;
        let end_offset = start_offset + nx * ny * self.bytes_per_voxel;

        let bytes = &mut self.data[start_offset..end_offset];
        unsafe {
            let ptr = bytes.as_mut_ptr() as *mut f32;
            Ok(core::slice::from_raw_parts_mut(ptr, nx * ny))
        }
    }

    /// Write a block with parallel encoding and file I/O
    #[cfg(all(feature = "parallel", feature = "std"))]
    pub fn write_block_parallel(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        use crate::encode::encode_block_parallel;
        use rayon::prelude::*;
        use std::os::unix::fs::FileExt;
        
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let base_offset = ((ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel) as u64;
        
        // Mark that parallel writes were used
        self.parallel_writes = true;
        
        // Encode in parallel and write to file
        let encoded_chunks = encode_block_parallel(&block.data, chunk_size, FileEndian::LittleEndian);
        
        // Write chunks in parallel using pwrite
        encoded_chunks.par_iter().for_each(|(chunk_idx, encoded)| {
            let offset = base_offset + (*chunk_idx * chunk_size * self.bytes_per_voxel) as u64;
            self.file.write_all_at(encoded, offset).unwrap();
        });
        
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
                    .map_err(|_| Error::Io)?;
                self.file.write_all(&self.data).map_err(|_| Error::Io)?;
            }
        }
        
        #[cfg(not(feature = "parallel"))]
        {
            self.file
                .seek(SeekFrom::Start(self.data_offset))
                .map_err(|_| Error::Io)?;
            self.file.write_all(&self.data).map_err(|_| Error::Io)?;
        }

        // Rewrite header
        self.file.seek(SeekFrom::Start(0)).map_err(|_| Error::Io)?;

        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        self.file.write_all(&header_bytes).map_err(|_| Error::Io)?;

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
}

#[cfg(feature = "mmap")]
impl MmapWriter {
    pub fn create(path: &str, header: Header) -> Result<Self, Error> {
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
            .open(path)
            .map_err(|_| Error::Io)?;

        file.set_len(total_size as u64).map_err(|_| Error::Io)?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        file.write_all(&header_bytes).map_err(|_| Error::Io)?;

        let mmap = unsafe { memmap2::MmapOptions::new().map_mut(&file).map_err(|_| Error::Mmap)? };

        let data_offset = header.data_offset();
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let bytes_per_voxel = match mode {
            Mode::Float32 => 4,
            Mode::Int16 => 2,
            Mode::Uint16 => 2,
            Mode::Int8 => 1,
            _ => return Err(Error::UnsupportedMode),
        };

        let shape = VolumeShape::new(
            header.nx as usize,
            header.ny as usize,
            header.nz as usize,
        );

        Ok(Self {
            mmap,
            header,
            data_offset,
            bytes_per_voxel,
            shape,
        })
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    pub fn write_block(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let start_offset = (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        let end_offset = start_offset + sx * sy * sz * self.bytes_per_voxel;

        if end_offset > self.mmap.len() {
            return Err(Error::BoundsError);
        }

        let data_bytes = unsafe {
            core::slice::from_raw_parts(
                block.data.as_ptr() as *const u8,
                block.data.len() * 4,
            )
        };

        self.mmap[start_offset..end_offset].copy_from_slice(data_bytes);
        Ok(())
    }

    /// Write a block with parallel encoding to memory-mapped region
    /// This is extremely fast as it requires no syscalls
    #[cfg(all(feature = "parallel", feature = "std"))]
    pub fn write_block_parallel(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        use rayon::prelude::*;
        
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let base_offset = self.data_offset + (ox + oy * nx + oz * nx * ny) * self.bytes_per_voxel;
        
        // Get raw pointer as usize for parallel writes
        // This is safe because each thread writes to a different region
        let mmap_ptr = self.mmap.as_mut_ptr() as usize;
        
        // Encode and write to mmap in parallel
        block.data
            .par_chunks(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start_offset = base_offset + chunk_idx * chunk_size * self.bytes_per_voxel;
                let ptr = (mmap_ptr + start_offset) as *mut u8;
                let dst = unsafe {
                    core::slice::from_raw_parts_mut(
                        ptr,
                        chunk.len() * self.bytes_per_voxel
                    )
                };
                
                // Encode chunk directly to mmap
                for (i, &val) in chunk.iter().enumerate() {
                    val.encode(dst, i * 4, FileEndian::LittleEndian);
                }
            });
        
        Ok(())
    }

    pub fn slice_mut(&mut self, z: usize) -> Result<&mut [f32], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        if z >= nz {
            return Err(Error::BoundsError);
        }

        let start_offset = self.data_offset + z * nx * ny * self.bytes_per_voxel;
        let end_offset = start_offset + nx * ny * self.bytes_per_voxel;

        let bytes = &mut self.mmap[start_offset..end_offset];
        unsafe {
            let ptr = bytes.as_mut_ptr() as *mut f32;
            Ok(core::slice::from_raw_parts_mut(ptr, nx * ny))
        }
    }

    pub fn finalize(&mut self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        self.mmap[0..1024].copy_from_slice(&header_bytes);
        self.mmap.flush().map_err(|_| Error::Io)?;
        Ok(())
    }
}