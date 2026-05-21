//! MRC file writer with block-based API

use crate::engine::block::{VolumeShape, VoxelBlock};
#[cfg(feature = "parallel")]
use crate::engine::codec::encode_block_parallel;
use crate::engine::codec::encode_slice;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::path::PathBuf;
use std::vec::Vec;

/// Builder for configuring and creating a new MRC file writer.
///
/// # Example
///
/// ```no_run
/// use mrc::{create, VoxelBlock};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut writer = create("output.mrc")
///         .shape([512, 512, 256])
///         .mode::<f32>()
///         .finish()?;
///
///     writer.write_block(&VoxelBlock::new(
///         [0, 0, 0], [512, 512, 1],
///         vec![0.0f32; 512 * 512],
///     ))?;
///     writer.finalize()?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct WriterBuilder {
    path: PathBuf,
    header: Header,
}

impl WriterBuilder {
    /// Create a new builder with default header values.
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
        self.header.mx = self.header.nx;
        self.header.my = self.header.ny;
        self.header.mz = self.header.nz;
        self
    }

    pub fn mode<T: Voxel>(mut self) -> Self {
        self.header.mode = T::MODE.as_i32();
        self
    }

    /// Set the cell dimensions in Angstroms.
    pub fn cell_lengths(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
        self.header.xlen = xlen;
        self.header.ylen = ylen;
        self.header.zlen = zlen;
        self
    }

    /// Set the space group number.
    pub fn ispg(mut self, ispg: i32) -> Self {
        self.header.ispg = ispg;
        self
    }

    /// Set the extended header type (4-byte ASCII identifier).
    pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
        self.header.set_exttyp(exttyp);
        self
    }

    /// Set the extended header size in bytes.
    pub fn nsymbt(mut self, nsymbt: i32) -> Self {
        self.header.nsymbt = nsymbt;
        self
    }

    /// Set the origin coordinates.
    pub fn origin(mut self, origin: [f32; 3]) -> Self {
        self.header.origin = origin;
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

/// MRC file writer using standard file I/O.
///
/// For most use cases, prefer creating via [`WriterBuilder`](crate::WriterBuilder)
/// or the [`create`](crate::create) convenience function.
///
/// The writer maintains an open file handle and writes data blocks directly
/// to disk. Call [`finalize`](Self::finalize) when done to ensure the header
/// is correctly rewritten.
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

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

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
            eprintln!(
                "Warning: Mode 3 (Int16Complex) is obsolete and should not be used for writing new files."
            );
        }
        if mode == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }
        let bytes_per_voxel = mode.byte_size();

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

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
    /// Supports arbitrary sub-blocks by scattering row-by-row when necessary.
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
        let b = self.bytes_per_voxel;
        let file_endian = self.header.detect_endian();

        // Fast path: full XY slab is contiguous in the file.
        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear =
                (ox as u64) + (oy as u64) * (nx as u64) + (oz as u64) * (nx as u64) * (ny as u64);
            let start_offset = self.data_offset + linear * (b as u64);
            let byte_len = (sx as u64) * (sy as u64) * (sz as u64) * (b as u64);
            let byte_len_usize = byte_len.try_into().map_err(|_| Error::BoundsError)?;
            let mut buffer = vec![0u8; byte_len_usize];
            encode_slice(&block.data, &mut buffer, file_endian);

            use std::io::{Seek, SeekFrom, Write};
            self.file.seek(SeekFrom::Start(start_offset))?;
            self.file.write_all(&buffer)?;
            return Ok(());
        }

        // Scatter path: write row by row.
        use std::io::{Seek, SeekFrom, Write};
        for z in 0..sz {
            for y in 0..sy {
                let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
                let file_offset = self.data_offset + (file_linear as u64) * (b as u64);
                let block_idx = y * sx + z * sx * sy;
                let row_values = &block.data[block_idx..block_idx + sx];
                let mut row_bytes = vec![0u8; sx * b];
                encode_slice(row_values, &mut row_bytes, file_endian);
                self.file.seek(SeekFrom::Start(file_offset))?;
                self.file.write_all(&row_bytes)?;
            }
        }
        Ok(())
    }

    /// Write a block of `u8` data by automatically widening to `u16` (Mode 6).
    ///
    /// The file must have been created with [`Mode::Uint16`]. Each `u8` voxel
    /// is widened to `u16` before writing, matching Python `mrcfile`'s
    /// auto-conversion behaviour for `np.uint8` data.
    ///
    /// # Errors
    /// Returns [`Error::ModeMismatch`] if the file mode is not `Uint16`.
    pub fn write_u8_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let widened = crate::engine::convert::convert_u8_slice_to_u16(&block.data);
        self.write_block(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data: widened,
        })
    }

    /// Write a block with parallel encoding and sequential file I/O.
    ///
    /// Encoding is performed in parallel using all available cores.
    /// File writes are performed sequentially to ensure cross-platform compatibility.
    ///
    /// For non-contiguous blocks (sub-XY slabs), this falls back to the serial
    /// [`write_block`](Self::write_block) implementation.
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
        let [ox, oy, _oz] = block.offset;
        let [sx, sy, _sz] = block.shape;

        // Parallel fast path only works for full XY slabs (contiguous in file).
        if ox != 0 || sx != nx || oy != 0 || sy != ny {
            return self.write_block(block);
        }

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let linear =
            (ox as u64) + (oy as u64) * (nx as u64) + (_oz as u64) * (nx as u64) * (ny as u64);
        let base_offset = self.data_offset + linear * (self.bytes_per_voxel as u64);
        let file_endian = self.header.detect_endian();

        // Encode in parallel
        let encoded_chunks = encode_block_parallel(&block.data, chunk_size, file_endian);

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
        let data: Vec<crate::f16> = block
            .data
            .iter()
            .map(|&v| crate::f16::from_f32(v))
            .collect();
        self.write_block::<crate::f16>(&VoxelBlock {
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
        update_header_stats_from_bytes(&mut self.header, &buf)?;
        Ok(())
    }
}

// ============================================================================
// Stats helpers
// ============================================================================

fn update_header_stats_from_bytes(header: &mut Header, bytes: &[u8]) -> Result<(), Error> {
    let endian = header.detect_endian();
    let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
    let (dmin, dmax, dmean, rms) = crate::engine::stats::compute_stats(bytes, mode, endian);
    header.dmin = dmin;
    header.dmax = dmax;
    header.dmean = dmean;
    header.rms = rms;
    Ok(())
}

// ============================================================================
// MmapWriter
// ============================================================================

/// Builder for configuring and creating a memory-mapped MRC file writer.
///
/// Memory-mapped writers are useful when you need to modify specific regions
/// of an existing file without rewriting the entire dataset.
///
/// # Example
///
/// ```no_run
/// use mrc::{create, VoxelBlock};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut writer = create("output.mrc")
///         .shape([512, 512, 256])
///         .mode::<f32>()
///         .mmap()
///         .finish()?;
///
///     writer.write_block(&VoxelBlock::new(
///         [0, 0, 0], [512, 512, 1],
///         vec![0.0f32; 512 * 512],
///     ))?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "mmap")]
#[derive(Debug)]
pub struct MmapWriterBuilder {
    path: PathBuf,
    header: Header,
}

#[cfg(feature = "mmap")]
impl MmapWriterBuilder {
    /// Create a new builder with default header values.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            header: Header::new(),
        }
    }

    /// Set the volume dimensions.
    ///
    /// Also synchronises `mx`, `my`, `mz` to match `nx`, `ny`, `nz`.
    pub fn shape(mut self, shape: [usize; 3]) -> Self {
        self.header.nx = shape[0] as i32;
        self.header.ny = shape[1] as i32;
        self.header.nz = shape[2] as i32;
        self.header.mx = self.header.nx;
        self.header.my = self.header.ny;
        self.header.mz = self.header.nz;
        self
    }

    pub fn mode<T: Voxel>(mut self) -> Self {
        self.header.mode = T::MODE.as_i32();
        self
    }

    /// Set the cell dimensions in Angstroms.
    pub fn cell_lengths(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
        self.header.xlen = xlen;
        self.header.ylen = ylen;
        self.header.zlen = zlen;
        self
    }

    /// Set the space group number.
    pub fn ispg(mut self, ispg: i32) -> Self {
        self.header.ispg = ispg;
        self
    }

    /// Set the extended header type (4-byte ASCII identifier).
    pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
        self.header.set_exttyp(exttyp);
        self
    }

    /// Set the extended header size in bytes.
    pub fn nsymbt(mut self, nsymbt: i32) -> Self {
        self.header.nsymbt = nsymbt;
        self
    }

    /// Set the origin coordinates.
    pub fn origin(mut self, origin: [f32; 3]) -> Self {
        self.header.origin = origin;
        self
    }

    pub fn finish(self) -> Result<MmapWriter, Error> {
        MmapWriter::create(self.path, self.header)
    }
}

/// Memory-mapped MRC file writer.
///
/// Writes data directly into a memory-mapped region, letting the OS handle
/// paging and flushing. This is efficient for large files and random-access
/// modifications, but requires the `mmap` feature.
///
/// For most use cases, prefer creating via [`MmapWriterBuilder`](crate::MmapWriterBuilder)
/// or chaining [`WriterBuilder::mmap`](crate::WriterBuilder::mmap).
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

        let total_size = header
            .data_offset()
            .checked_add(header.data_size().ok_or(Error::InvalidHeader)?)
            .ok_or(Error::InvalidHeader)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

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
            eprintln!(
                "Warning: Mode 3 (Int16Complex) is obsolete and should not be used for writing new files."
            );
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

    /// Write a block of `u8` data by automatically widening to `u16` (Mode 6).
    ///
    /// The file must have been created with [`Mode::Uint16`]. Each `u8` voxel
    /// is widened to `u16` before writing, matching Python `mrcfile`'s
    /// auto-conversion behaviour for `np.uint8` data.
    ///
    /// # Errors
    /// Returns [`Error::ModeMismatch`] if the file mode is not `Uint16`.
    pub fn write_u8_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let widened = crate::engine::convert::convert_u8_slice_to_u16(&block.data);
        self.write_block(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data: widened,
        })
    }

    /// Write a block of voxels to the memory-mapped file.
    ///
    /// The type `T` must match the file's voxel mode exactly.
    /// Supports arbitrary sub-blocks by scattering row-by-row when necessary.
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
        let b = self.bytes_per_voxel;
        let file_endian = self.header.detect_endian();

        // Fast path: full XY slab is contiguous in the file.
        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear = self
                .shape
                .checked_linear_index(block.offset)
                .ok_or(Error::BoundsError)?;
            let start_offset = self
                .data_offset
                .checked_add(linear.checked_mul(b).ok_or(Error::BoundsError)?)
                .ok_or(Error::BoundsError)?;
            let count = sx
                .checked_mul(sy)
                .and_then(|v| v.checked_mul(sz))
                .ok_or(Error::BoundsError)?;
            let byte_len = count.checked_mul(b).ok_or(Error::BoundsError)?;
            let end_offset = start_offset
                .checked_add(byte_len)
                .ok_or(Error::BoundsError)?;
            if end_offset > self.mmap.len() {
                return Err(Error::BoundsError);
            }
            encode_slice(
                &block.data,
                &mut self.mmap[start_offset..end_offset],
                file_endian,
            );
            return Ok(());
        }

        // Scatter path: write row by row directly into the mmap.
        for z in 0..sz {
            for y in 0..sy {
                let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
                let file_start = self.data_offset + file_linear * b;
                let row_end = file_start + sx * b;
                if row_end > self.mmap.len() {
                    return Err(Error::BoundsError);
                }
                let block_idx = y * sx + z * sx * sy;
                let row_values = &block.data[block_idx..block_idx + sx];
                encode_slice(row_values, &mut self.mmap[file_start..row_end], file_endian);
            }
        }
        Ok(())
    }

    /// Write a block with parallel encoding to memory-mapped region.
    ///
    /// For non-contiguous blocks (sub-XY slabs), this falls back to the serial
    /// [`write_block`](Self::write_block) implementation.
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

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, _oz] = block.offset;
        let [sx, sy, _sz] = block.shape;

        // Parallel fast path only works for full XY slabs (contiguous in file).
        if ox != 0 || sx != nx || oy != 0 || sy != ny {
            return self.write_block(block);
        }

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let linear = self
            .shape
            .checked_linear_index(block.offset)
            .ok_or(Error::BoundsError)?;
        let base_offset = self
            .data_offset
            .checked_add(
                linear
                    .checked_mul(self.bytes_per_voxel)
                    .ok_or(Error::BoundsError)?,
            )
            .ok_or(Error::BoundsError)?;
        let file_endian = self.header.detect_endian();

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
        let data: Vec<crate::f16> = block
            .data
            .iter()
            .map(|&v| crate::f16::from_f32(v))
            .collect();
        self.write_block::<crate::f16>(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data,
        })
    }

    /// Scan the written data block and update header statistics.
    ///
    /// Unlike [`Writer::update_header_stats`], this does not need to read from
    /// disk because the data is already accessible via the memory map.
    pub fn update_header_stats(&mut self) -> Result<(), Error> {
        let data_size = self.header.data_size().ok_or(Error::InvalidHeader)?;
        let end = self
            .data_offset
            .checked_add(data_size)
            .ok_or(Error::InvalidHeader)?;
        if end <= self.mmap.len() {
            update_header_stats_from_bytes(&mut self.header, &self.mmap[self.data_offset..end])?;
        }
        Ok(())
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

// -------------------------------------------------------------------------
// Compressed writer (gzip / bzip2)
// -------------------------------------------------------------------------

/// Compression backend trait for [`CompressedWriter`].
///
/// Implementations of this trait plug into [`CompressedWriter`] to provide
/// a specific compression algorithm (e.g. gzip via [`GzipCompressor`](crate::io::gzip::GzipCompressor),
/// bzip2 via [`Bzip2Compressor`](crate::io::bzip2::Bzip2Compressor)).
///
/// The trait has a single method so that [`CompressedWriter`] can remain
/// generic without carrying runtime state for the compressor.
pub trait Compressor {
    /// Compress `data` and return the compressed bytes.
    fn compress(data: &[u8]) -> Result<Vec<u8>, Error>;
}

/// MRC file writer that buffers the entire file in memory and compresses on
/// [`finalize`](CompressedWriter::finalize).
///
/// Compressed formats (gzip, bzip2) do not support random access, so this
/// writer accumulates header and voxel data in a `Vec<u8>` during construction
/// and [`write_block`] calls. Only when [`finalize`](CompressedWriter::finalize)
/// is invoked does it assemble the full file, compress it via [`Compressor::compress`],
/// and write the result to disk in one shot.
///
/// This design matches the behaviour of the reference Python `mrcfile` library.
/// For large volumes that do not fit in RAM, prefer [`Writer`] (uncompressed)
/// or write to an uncompressed file and compress it afterwards.
///
/// Concrete type aliases are provided for convenience:
/// * [`GzipWriter`](crate::GzipWriter) = `CompressedWriter<GzipCompressor>`
/// * [`Bzip2Writer`](crate::Bzip2Writer) = `CompressedWriter<Bzip2Compressor>`
#[derive(Debug)]
pub struct CompressedWriter<C: Compressor> {
    header: Header,
    data: Vec<u8>,
    path: std::path::PathBuf,
    data_offset: usize,
    bytes_per_voxel: usize,
    shape: VolumeShape,
    _marker: std::marker::PhantomData<C>,
}

impl<C: Compressor> CompressedWriter<C> {
    /// Create a new compressed MRC file.
    pub fn create<P: AsRef<std::path::Path>>(path: P, header: Header) -> Result<Self, Error> {
        let mut header = header;
        header.set_file_endian(FileEndian::LittleEndian);
        header.validate_detailed()?;

        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let data = vec![0u8; data_size];

        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }
        let bytes_per_voxel = mode.byte_size();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            header,
            data,
            path: path.as_ref().to_path_buf(),
            data_offset: header.data_offset(),
            bytes_per_voxel,
            shape,
            _marker: std::marker::PhantomData,
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

    /// Write a block of voxels to the file.
    ///
    /// The type `T` must match the file's voxel mode exactly.
    /// Supports arbitrary sub-blocks by scattering row-by-row when necessary.
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
        let b = self.bytes_per_voxel;
        let file_endian = self.header.detect_endian();

        // Fast path: full XY slab is contiguous in the buffer.
        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear = oz * nx * ny;
            let start_byte = self.data_offset + linear * b;
            let byte_len = sx * sy * sz * b;
            let dst = &mut self.data[start_byte..start_byte + byte_len];
            crate::engine::codec::encode_slice(&block.data, dst, file_endian);
            return Ok(());
        }

        // Scatter path: write row by row directly into self.data.
        for z in 0..sz {
            for y in 0..sy {
                let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
                let file_start = self.data_offset + file_linear * b;
                let block_idx = y * sx + z * sx * sy;
                let row_values = &block.data[block_idx..block_idx + sx];
                let row_end = file_start + sx * b;
                crate::engine::codec::encode_slice(
                    row_values,
                    &mut self.data[file_start..row_end],
                    file_endian,
                );
            }
        }
        Ok(())
    }

    /// Write a block of `u8` data by automatically widening to `u16` (Mode 6).
    ///
    /// The file must have been created with [`Mode::Uint16`]. Each `u8` voxel
    /// is widened to `u16` before writing, matching Python `mrcfile`'s
    /// auto-conversion behaviour for `np.uint8` data.
    pub fn write_u8_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let widened = crate::engine::convert::convert_u8_slice_to_u16(&block.data);
        self.write_block(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data: widened,
        })
    }

    pub fn finalize(self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);

        let ext_size = self.header.nsymbt as usize;
        let mut file_bytes = Vec::with_capacity(1024 + ext_size + self.data.len());
        file_bytes.extend_from_slice(&header_bytes);
        if ext_size > 0 {
            file_bytes.resize(file_bytes.len() + ext_size, 0);
        }
        file_bytes.extend_from_slice(&self.data);

        let compressed = C::compress(&file_bytes)?;
        std::fs::write(&self.path, compressed)?;
        Ok(())
    }
}
