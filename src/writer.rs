//! MRC file writer with block-based API

use crate::engine::block::{SliceAccess, VolumeShape, VoxelBlock};
use crate::engine::codec::{encode_block_parallel, encode_slice};
use crate::engine::endian::FileEndian;
use crate::mode::{Float32Complex, Int16Complex, Voxel};
use crate::{Error, Header, Mode};

use std::path::PathBuf;
use std::vec::Vec;

#[derive(Debug)]
pub struct WriterBuilder {
    path: PathBuf,
    header: Header,
}

impl WriterBuilder {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            header: Header::new(),
        }
    }

    pub fn shape(mut self, shape: [usize; 3]) -> Self {
        self.header.nx = shape[0] as i32;
        self.header.ny = shape[1] as i32;
        self.header.nz = shape[2] as i32;
        self
    }

    pub fn mode<T: Voxel>(mut self) -> Self {
        self.header.mode = T::MODE.as_i32();
        self
    }

    pub fn finish(self) -> Result<Writer, Error> {
        Writer::create(self.path, self.header)
    }

    /// Switch to building a memory-mapped writer.
    #[cfg(feature = "mmap")]
    pub fn mmap(self) -> MmapWriterBuilder {
        MmapWriterBuilder {
            path: self.path,
            header: self.header,
        }
    }
}

#[derive(Debug)]
pub struct Writer {
    file: std::fs::File,
    header: Header,
    data_offset: u64,
    bytes_per_voxel: usize,
    shape: VolumeShape,
}

impl Writer {
    /// Create a new MRC file from a pre-built header.
    ///
    /// The header's endianness is forced to little-endian per crate policy.
    /// For most use cases, prefer [`WriterBuilder`](crate::WriterBuilder).
    pub fn create<P: AsRef<std::path::Path>>(path: P, mut header: Header) -> Result<Self, Error> {
        use std::io::Write;

        // New files are always little-endian per crate policy
        header.set_file_endian(FileEndian::LittleEndian);

        header.validate_detailed()?;

        let mut file = std::fs::File::create(path)?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        file.write_all(&header_bytes)?;

        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            let zeros = vec![0u8; ext_size];
            file.write_all(&zeros)?;
        }

        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Int16Complex {
            eprintln!("Warning: Mode 3 (Int16Complex) is obsolete and should not be used for writing new files.");
        }
        if mode == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }
        let bytes_per_voxel = mode.byte_size();

        let shape =
            VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            file,
            header,
            data_offset,
            bytes_per_voxel,
            shape,
        })
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    /// Write a block of voxels to the file.
    ///
    /// The type `T` must match the file's voxel mode exactly.
    pub fn write_block<T: Voxel>(&mut self, block: &VoxelBlock<T>) -> Result<(), Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        let linear = (ox as u64)
            + (oy as u64) * (nx as u64)
            + (oz as u64) * (nx as u64) * (ny as u64);
        let start_offset = self.data_offset
            + linear * (self.bytes_per_voxel as u64);
        let byte_len = (sx as u64) * (sy as u64) * (sz as u64) * (self.bytes_per_voxel as u64);

        // Encode to a temporary buffer and write directly
        let byte_len_usize = byte_len.try_into().map_err(|_| Error::BoundsError)?;
        let mut buffer = vec![0u8; byte_len_usize];
        let file_endian = self.header.detect_endian();
        encode_slice(&block.data, &mut buffer, file_endian);

        use std::io::{Seek, SeekFrom, Write};
        self.file.seek(SeekFrom::Start(start_offset))?;
        self.file.write_all(&buffer)?;

        Ok(())
    }

    /// Write a block with parallel encoding and sequential file I/O.
    ///
    /// Encoding is performed in parallel using all available cores.
    /// File writes are performed sequentially to ensure cross-platform compatibility.
    #[cfg(feature = "parallel")]
    pub fn write_block_parallel<T: Voxel>(&mut self, block: &VoxelBlock<T>) -> Result<(), Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let linear = (ox as u64)
            + (oy as u64) * (nx as u64)
            + (oz as u64) * (nx as u64) * (ny as u64);
        let base_offset = self.data_offset
            + linear * (self.bytes_per_voxel as u64);
        let file_endian = self.header.detect_endian();

        // Encode in parallel
        let encoded_chunks =
            encode_block_parallel(&block.data, chunk_size, file_endian);

        // Write chunks sequentially (cross-platform)
        use std::io::{Seek, SeekFrom, Write};
        for (chunk_idx, encoded) in encoded_chunks {
            let offset = base_offset
                + (chunk_idx as u64) * (chunk_size as u64) * (self.bytes_per_voxel as u64);
            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(&encoded)?;
        }

        Ok(())
    }

    /// Write an `f32` block to a Float16 file.
    ///
    /// This is a convenience method for the common case of writing f32 data
    /// to a half-precision MRC file.
    #[cfg(feature = "f16")]
    pub fn write_f16_from_f32(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        if self.mode() != Mode::Float16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Float16,
            });
        }
        let data: Vec<f16> = block.data.iter().map(|&v| v as f16).collect();
        self.write_block::<f16>(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data,
        })
    }

    pub fn finalize(&mut self) -> Result<(), Error> {
        use std::io::{Seek, SeekFrom, Write};

        // Rewrite header
        self.file.seek(SeekFrom::Start(0))?;

        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        self.file.write_all(&header_bytes)?;

        Ok(())
    }

    /// Scan the written data block and update `dmin`, `dmax`, `dmean` and `rms`
    /// in the header to match the actual file contents.
    ///
    /// This is an optional convenience; it reads the entire data block back
    /// from disk, so it can be expensive for large files.
    pub fn update_header_stats(&mut self) -> Result<(), Error> {
        use std::io::{Read, Seek, SeekFrom};
        let data_size = self.header.data_size().ok_or(Error::InvalidHeader)?;
        self.file.seek(SeekFrom::Start(self.data_offset))?;
        let mut buf = vec![0u8; data_size];
        self.file.read_exact(&mut buf)?;
        update_header_stats_from_bytes(&mut self.header, &buf);
        Ok(())
    }
}

// ============================================================================
// Stats helpers
// ============================================================================

fn stats_real<T>(data: &[T]) -> (f32, f32, f32, f32)
where
    T: Copy + Into<f64>,
{
    if data.is_empty() {
        return (0.0, -1.0, -2.0, -1.0);
    }
    let iter = || data.iter().copied().map(Into::<f64>::into);
    let min = iter().fold(f64::INFINITY, f64::min) as f32;
    let max = iter().fold(f64::NEG_INFINITY, f64::max) as f32;
    let sum: f64 = iter().sum();
    let mean = (sum / data.len() as f64) as f32;
    let variance: f64 = iter().map(|v| {
        let d = v - mean as f64;
        d * d
    }).sum::<f64>() / data.len() as f64;
    let rms = variance.sqrt() as f32;
    (min, max, mean, rms)
}

fn rms_complex_f32(data: &[Float32Complex]) -> f32 {
    if data.is_empty() {
        return -1.0;
    }
    let mean_real = data.iter().map(|c| c.real as f64).sum::<f64>() / data.len() as f64;
    let mean_imag = data.iter().map(|c| c.imag as f64).sum::<f64>() / data.len() as f64;
    let variance: f64 = data.iter().map(|c| {
        let dr = c.real as f64 - mean_real;
        let di = c.imag as f64 - mean_imag;
        dr * dr + di * di
    }).sum::<f64>() / data.len() as f64;
    variance.sqrt() as f32
}

fn rms_complex_i16(data: &[Int16Complex]) -> f32 {
    if data.is_empty() {
        return -1.0;
    }
    let mean_real = data.iter().map(|c| c.real as f64).sum::<f64>() / data.len() as f64;
    let mean_imag = data.iter().map(|c| c.imag as f64).sum::<f64>() / data.len() as f64;
    let variance: f64 = data.iter().map(|c| {
        let dr = c.real as f64 - mean_real;
        let di = c.imag as f64 - mean_imag;
        dr * dr + di * di
    }).sum::<f64>() / data.len() as f64;
    variance.sqrt() as f32
}

fn update_header_stats_from_bytes(header: &mut Header, bytes: &[u8]) {
    use crate::engine::codec::decode_slice;
    let endian = header.detect_endian();
    match Mode::from_i32(header.mode) {
        Some(Mode::Float32) => {
            let data = decode_slice::<f32>(bytes, endian);
            let (min, max, mean, rms) = stats_real(&data);
            header.dmin = min;
            header.dmax = max;
            header.dmean = mean;
            header.rms = rms;
        }
        Some(Mode::Int16) => {
            let data = decode_slice::<i16>(bytes, endian);
            let (min, max, mean, rms) = stats_real(&data);
            header.dmin = min;
            header.dmax = max;
            header.dmean = mean;
            header.rms = rms;
        }
        Some(Mode::Uint16) => {
            let data = decode_slice::<u16>(bytes, endian);
            let (min, max, mean, rms) = stats_real(&data);
            header.dmin = min;
            header.dmax = max;
            header.dmean = mean;
            header.rms = rms;
        }
        Some(Mode::Int8) => {
            let data = decode_slice::<i8>(bytes, endian);
            let (min, max, mean, rms) = stats_real(&data);
            header.dmin = min;
            header.dmax = max;
            header.dmean = mean;
            header.rms = rms;
        }
        Some(Mode::Float32Complex) => {
            let data = decode_slice::<Float32Complex>(bytes, endian);
            header.rms = rms_complex_f32(&data);
        }
        Some(Mode::Int16Complex) => {
            let data = decode_slice::<Int16Complex>(bytes, endian);
            header.rms = rms_complex_i16(&data);
        }
        #[cfg(feature = "f16")]
        Some(Mode::Float16) => {
            let data = decode_slice::<f16>(bytes, endian);
            let data_f32: Vec<f32> = data.iter().map(|&v| v as f32).collect();
            let (min, max, mean, rms) = stats_real(&data_f32);
            header.dmin = min;
            header.dmax = max;
            header.dmean = mean;
            header.rms = rms;
        }
        _ => {}
    }
}

// ============================================================================
// MmapWriter
// ============================================================================

#[cfg(feature = "mmap")]
#[derive(Debug)]
pub struct MmapWriterBuilder {
    path: PathBuf,
    header: Header,
}

#[cfg(feature = "mmap")]
impl MmapWriterBuilder {
    pub fn shape(mut self, shape: [usize; 3]) -> Self {
        self.header.nx = shape[0] as i32;
        self.header.ny = shape[1] as i32;
        self.header.nz = shape[2] as i32;
        self
    }

    pub fn mode<T: Voxel>(mut self) -> Self {
        self.header.mode = T::MODE.as_i32();
        self
    }

    pub fn finish(self) -> Result<MmapWriter, Error> {
        MmapWriter::create(self.path, self.header)
    }
}

#[cfg(feature = "mmap")]
#[derive(Debug)]
pub struct MmapWriter {
    mmap: memmap2::MmapMut,
    header: Header,
    data_offset: usize,
    bytes_per_voxel: usize,
    shape: VolumeShape,
}

#[cfg(feature = "mmap")]
impl MmapWriter {
    /// Create a new memory-mapped MRC file from a pre-built header.
    ///
    /// The header's endianness is forced to little-endian per crate policy.
    /// For most use cases, prefer [`MmapWriterBuilder`](crate::MmapWriterBuilder).
    pub fn create<P: AsRef<std::path::Path>>(path: P, mut header: Header) -> Result<Self, Error> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // New files are always little-endian per crate policy
        header.set_file_endian(FileEndian::LittleEndian);

        header.validate_detailed()?;

        let total_size = header.data_offset()
            .checked_add(header.data_size().ok_or(Error::InvalidHeader)?)
            .ok_or(Error::InvalidHeader)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            ?;

        file.set_len(total_size as u64)?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        file.write_all(&header_bytes)?;

        // Explicitly zero the extended header region so the mmap does not see
        // uninitialised bytes (the data region is implicitly zero because the
        // file was truncated before set_len).
        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            let zeros = vec![0u8; ext_size];
            file.write_all(&zeros)?;
        }

        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .map_mut(&file)
                .map_err(|_| Error::Mmap)?
        };

        let data_offset = header.data_offset();
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Int16Complex {
            eprintln!("Warning: Mode 3 (Int16Complex) is obsolete and should not be used for writing new files.");
        }
        if mode == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }
        let bytes_per_voxel = mode.byte_size();

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

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

    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    /// Write a block of voxels to the memory-mapped file.
    ///
    /// The type `T` must match the file's voxel mode exactly.
    pub fn write_block<T: Voxel>(&mut self, block: &VoxelBlock<T>) -> Result<(), Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [sx, sy, sz] = block.shape;

        let linear = self.shape.checked_linear_index(block.offset).ok_or(Error::BoundsError)?;
        let start_offset = self.data_offset.checked_add(
            linear.checked_mul(self.bytes_per_voxel).ok_or(Error::BoundsError)?
        ).ok_or(Error::BoundsError)?;
        let count = sx.checked_mul(sy).and_then(|v| v.checked_mul(sz))
            .ok_or(Error::BoundsError)?;
        let byte_len = count.checked_mul(self.bytes_per_voxel).ok_or(Error::BoundsError)?;
        let end_offset = start_offset.checked_add(byte_len).ok_or(Error::BoundsError)?;

        if end_offset > self.mmap.len() {
            return Err(Error::BoundsError);
        }

        let file_endian = self.header.detect_endian();
        encode_slice(
            &block.data,
            &mut self.mmap[start_offset..end_offset],
            file_endian,
        );
        Ok(())
    }

    /// Write a block with parallel encoding to memory-mapped region
    #[cfg(feature = "parallel")]
    pub fn write_block_parallel<T: Voxel>(&mut self, block: &VoxelBlock<T>) -> Result<(), Error> {
        use rayon::prelude::*;

        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let linear = self.shape.checked_linear_index(block.offset).ok_or(Error::BoundsError)?;
        let base_offset = self.data_offset.checked_add(
            linear.checked_mul(self.bytes_per_voxel).ok_or(Error::BoundsError)?
        ).ok_or(Error::BoundsError)?;
        let file_endian = self.header.detect_endian();

        // Get raw pointer as usize for parallel writes
        let mmap_ptr = self.mmap.as_mut_ptr() as usize;

        // Encode and write to mmap in parallel
        block
            .data
            .par_chunks(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start_offset = base_offset
                    + chunk_idx * chunk_size * self.bytes_per_voxel;
                let ptr = (mmap_ptr + start_offset) as *mut u8;
                let dst = unsafe {
                    core::slice::from_raw_parts_mut(ptr, chunk.len() * self.bytes_per_voxel)
                };

                encode_slice(chunk, dst, file_endian);
            });

        Ok(())
    }

    /// Write an `f32` block to a Float16 file.
    #[cfg(feature = "f16")]
    pub fn write_f16_from_f32(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        if self.mode() != Mode::Float16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Float16,
            });
        }
        let data: Vec<f16> = block.data.iter().map(|&v| v as f16).collect();
        self.write_block::<f16>(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data,
        })
    }

    /// Scan the written data block and update header statistics.
    ///
    /// Unlike [`Writer::update_header_stats`], this does not need to read from
    /// disk because the data is already accessible via the memory map.
    pub fn update_header_stats(&mut self) {
        let data_size = self.header.data_size().unwrap_or(0);
        let end = self.data_offset + data_size;
        if end <= self.mmap.len() {
            update_header_stats_from_bytes(&mut self.header, &self.mmap[self.data_offset..end]);
        }
    }
}

#[cfg(feature = "mmap")]
impl SliceAccess for MmapWriter {
    fn slice<T: crate::engine::codec::EndianCodec>(&self, z: usize) -> Result<&[T], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        if z >= nz {
            return Err(Error::BoundsError);
        }

        if core::mem::size_of::<T>() != self.bytes_per_voxel {
            return Err(Error::TypeMismatch {
                expected: self.bytes_per_voxel,
                actual: core::mem::size_of::<T>(),
            });
        }

        let start_offset = self.data_offset + z * nx * ny * self.bytes_per_voxel;
        let end_offset = start_offset + nx * ny * self.bytes_per_voxel;

        if start_offset % core::mem::align_of::<T>() != 0 {
            return Err(Error::InvalidHeader);
        }

        let bytes = &self.mmap[start_offset..end_offset];
        unsafe {
            let ptr = bytes.as_ptr() as *const T;
            Ok(core::slice::from_raw_parts(ptr, nx * ny))
        }
    }

    fn slice_mut<T: crate::engine::codec::EndianCodec>(
        &mut self,
        z: usize,
    ) -> Result<&mut [T], Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        if z >= nz {
            return Err(Error::BoundsError);
        }

        if core::mem::size_of::<T>() != self.bytes_per_voxel {
            return Err(Error::TypeMismatch {
                expected: self.bytes_per_voxel,
                actual: core::mem::size_of::<T>(),
            });
        }

        let start_offset = self.data_offset + z * nx * ny * self.bytes_per_voxel;
        let end_offset = start_offset + nx * ny * self.bytes_per_voxel;

        if start_offset % core::mem::align_of::<T>() != 0 {
            return Err(Error::InvalidHeader);
        }

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
        self.mmap.flush()?;
        Ok(())
    }
}
