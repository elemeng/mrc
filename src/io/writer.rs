//! MRC file writer with block-based API.
//!
//! Provides [`Writer`] (standard file I/O), [`MmapWriter`](crate::MmapWriter)
//! (memory-mapped, requires `mmap`), and [`CompressedWriter`] (gzip/bzip2 backend).
//! Use [`WriterBuilder`] or the [`create`](crate::create) convenience function
//! to construct a writer.

macro_rules! write_u8_block_body {
    ($self:ident, $block:ident) => {{
        if $self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: $self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let widened = crate::engine::convert::convert_u8_slice_to_u16(&$block.data);
        $self.write_block(&VoxelBlock {
            offset: $block.offset,
            shape: $block.shape,
            data: widened,
        })
    }};
}

#[cfg_attr(not(feature = "f16"), allow(unused_macros))]
macro_rules! write_f16_from_f32_body {
    ($self:ident, $block:ident) => {{
        if $self.mode() != Mode::Float16 {
            return Err(Error::ModeMismatch {
                file_mode: $self.mode(),
                requested_mode: Mode::Float16,
            });
        }
        let data: Vec<crate::f16> = $block
            .data
            .iter()
            .map(|&v| crate::f16::from_f32(v))
            .collect();
        $self.write_block::<crate::f16>(&VoxelBlock {
            offset: $block.offset,
            shape: $block.shape,
            data,
        })
    }};
}

macro_rules! write_u4_block_body {
    ($self:ident, $block:ident) => {{
        if $self.mode() != Mode::Packed4Bit {
            return Err(Error::ModeMismatch {
                file_mode: $self.mode(),
                requested_mode: Mode::Packed4Bit,
            });
        }
        if !$self.shape().contains_block($block.offset, $block.shape) {
            return Err(Error::BoundsError);
        }
        for &v in &$block.data {
            if v > 15 {
                return Err(crate::Error::TypeMismatch {
                    expected: 4,
                    actual: 8,
                });
            }
        }
        let nx = $block.shape[0];
        let ny = $block.shape[1];
        let nz = $block.shape[2];
        let packed = crate::engine::convert::pack_u8_to_u4_bytes(&$block.data, nx, ny * nz);
        $self.write_block_bytes(&packed, $block.offset, $block.shape)?;
        Ok(())
    }};
}

use crate::engine::block::{VolumeShape, VoxelBlock};
#[cfg(feature = "parallel")]
use crate::engine::codec::encode_block_parallel;
use crate::engine::codec::encode_slice;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::path::PathBuf;
use std::vec::Vec;

macro_rules! builder_setters {
    () => {
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

        /// Set the voxel data mode.
        pub fn mode<T: Voxel>(mut self) -> Self {
            self.header.mode = T::MODE.as_i32();
            self
        }

        /// Set the MRC mode by raw integer value (for modes without a [`Voxel`] impl).
        ///
        /// This is primarily useful for [`Mode::Packed4Bit`] (mode 101) which does not
        /// implement `Voxel`.  Invalid mode constants are caught by header validation
        /// at `finish()` time.
        pub fn mode_raw(mut self, mode: i32) -> Self {
            self.header.mode = mode;
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
    };
}

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
///     )?)?;
///     writer.finalize()?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct WriterBuilder {
    path: PathBuf,
    header: Header,
    ext_header: Vec<u8>,
}

impl WriterBuilder {
    /// Create a new builder with default header values.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            header: Header::new(),
            ext_header: Vec::new(),
        }
    }

    builder_setters!();

    /// Set the extended header bytes.
    ///
    /// When provided, `nsymbt` is automatically updated to match the byte
    /// length. Pass an empty `Vec` (or omit) to write zeros for the extended
    /// header region.
    pub fn ext_header_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.header.nsymbt = bytes.len() as i32;
        self.ext_header = bytes;
        self
    }

    /// Consume the builder and create a standard file-backed [`Writer`].
    ///
    /// The file is created (or truncated) and the header + extended header
    /// are written immediately. Voxel data can then be written with
    /// [`write_block`](Writer::write_block).
    ///
    /// # Errors
    /// Returns [`Error::InvalidHeaderDetailed`] if the header fails validation.
    /// Returns [`Error::Io`] if the file cannot be created or written.
    pub fn finish(self) -> Result<Writer, Error> {
        Writer::create(self.path, self.header, &self.ext_header)
    }

    /// Build a memory-mapped writer.
    ///
    /// Equivalent to [`finish`](Self::finish) but creates an [`MmapWriter`]
    /// instead of a [`Writer`].
    #[cfg(feature = "mmap")]
    pub fn finish_mmap(self) -> Result<MmapWriter, Error> {
        MmapWriter::create(self.path, self.header, &self.ext_header)
    }

    /// Build a gzip-compressed writer.
    ///
    /// Because gzip does not support random access, the entire file is buffered
    /// in memory and compressed only on finalize.
    /// For large volumes consider using [`finish`](Self::finish) instead.
    #[cfg(feature = "gzip")]
    pub fn finish_gzip(self) -> Result<crate::GzipWriter, Error> {
        CompressedWriter::create(self.path, self.header, &self.ext_header)
    }

    /// Build a bzip2-compressed writer.
    ///
    /// Because bzip2 does not support random access, the entire file is buffered
    /// in memory and compressed only on finalize.
    /// For large volumes consider using [`finish`](Self::finish) instead.
    #[cfg(feature = "bzip2")]
    pub fn finish_bzip2(self) -> Result<crate::Bzip2Writer, Error> {
        CompressedWriter::create(self.path, self.header, &self.ext_header)
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
    pub(crate) fn create<P: AsRef<std::path::Path>>(
        path: P,
        mut header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
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
            if ext_header.len() >= ext_size {
                file.write_all(&ext_header[..ext_size])?;
            } else {
                // Pad with zeros if provided bytes are shorter than nsymbt
                file.write_all(ext_header)?;
                let remaining = ext_size - ext_header.len();
                let zeros = vec![0u8; remaining];
                file.write_all(&zeros)?;
            }
        }

        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Int16Complex {
            eprintln!(
                "Warning: Mode 3 (Int16Complex) is obsolete and should not be used for writing new files."
            );
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

    /// Volume dimensions for this writer.
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode for this writer.
    ///
    /// Falls back to [`Mode::Float32`] if the header mode value is not recognised.
    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    /// Reference to the current header.
    ///
    /// Modify header fields before calling [`finalize`](Self::finalize) to
    /// change what gets written to disk.
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

        // Fast path: full XY slab is contiguous in the file.
        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear =
                (ox as u64) + (oy as u64) * (nx as u64) + (oz as u64) * (nx as u64) * (ny as u64);
            let start_offset = self.data_offset + linear * (b as u64);
            let byte_len = (sx as u64) * (sy as u64) * (sz as u64) * (b as u64);
            let byte_len_usize = byte_len.try_into().map_err(|_| Error::BoundsError)?;
            let mut buffer = vec![0u8; byte_len_usize];
            encode_slice(&block.data, &mut buffer, file_endian)?;

            use std::io::{Seek, SeekFrom, Write};
            self.file.seek(SeekFrom::Start(start_offset))?;
            self.file.write_all(&buffer)?;
            return Ok(());
        }

        // Scatter path: write row by row.
        // Pre-allocate a single row buffer to avoid per-row allocation churn.
        let mut row_bytes = vec![0u8; sx * b];
        use std::io::{Seek, SeekFrom, Write};
        for z in 0..sz {
            for y in 0..sy {
                let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
                let file_offset = self.data_offset + (file_linear as u64) * (b as u64);
                let block_idx = y * sx + z * sx * sy;
                if block_idx + sx > block.data.len() {
                    return Err(Error::BoundsError);
                }
                let row_values = &block.data[block_idx..block_idx + sx];
                encode_slice(row_values, &mut row_bytes, file_endian)?;
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
        write_u8_block_body!(self, block)
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
        let [ox, oy, oz] = block.offset;
        let [sx, sy, _sz] = block.shape;

        // Parallel fast path only works for full XY slabs (contiguous in file).
        if ox != 0 || sx != nx || oy != 0 || sy != ny {
            return self.write_block(block);
        }

        let chunk_size = 1024 * 1024; // 1M voxels per chunk
        let linear =
            (ox as u64) + (oy as u64) * (nx as u64) + (oz as u64) * (nx as u64) * (ny as u64);
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
        write_f16_from_f32_body!(self, block)
    }

    /// Write a block of `u8` data (0–15 per voxel) by packing to 4-bit (Mode 101).
    ///
    /// The file must have been created with [`Mode::Packed4Bit`]. Each `u8` value
    /// is checked to be in the range 0–15; values exceeding 15 produce an error.
    pub fn write_u4_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        write_u4_block_body!(self, block)
    }

    /// Write raw packed bytes at the given block offset.
    ///
    /// Internal helper used by [`write_u4_block`](Self::write_u4_block).
    fn write_block_bytes(
        &mut self,
        packed: &[u8],
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<(), Error> {
        use std::io::{Seek, SeekFrom, Write};
        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;
        let file_row_bytes = nx.div_ceil(2);
        let block_row_bytes = sx.div_ceil(2);

        debug_assert!(ox == 0, "write_block_bytes requires ox == 0");

        // Fast path: full XY slab is contiguous.
        if sx == nx && oy == 0 && sy == ny {
            let slice_bytes = ny * file_row_bytes;
            let start_offset = (self.data_offset as usize) + oz * slice_bytes;
            let byte_len = sz * slice_bytes;
            let end_offset = start_offset + byte_len;
            if end_offset > (self.data_offset as usize) + self.header.data_size().unwrap_or(0) {
                return Err(Error::BoundsError);
            }
            self.file.seek(SeekFrom::Start(start_offset as u64))?;
            self.file.write_all(&packed[..byte_len])?;
            return Ok(());
        }

        // Scatter path: write row by row.
        for z in 0..sz {
            for y in 0..sy {
                let vol_row = (oz + z) * ny + (oy + y);
                let file_offset = (self.data_offset as usize) + vol_row * file_row_bytes;
                let packed_start = (y + z * sy) * block_row_bytes;
                let packed_end = packed_start + block_row_bytes;
                if packed_end > packed.len() {
                    return Err(Error::BoundsError);
                }
                self.file.seek(SeekFrom::Start(file_offset as u64))?;
                self.file.write_all(&packed[packed_start..packed_end])?;
            }
        }
        Ok(())
    }

    /// Finalize the MRC file by rewriting the header at the beginning of the file.
    ///
    /// This must be called after all [`write_block`](Self::write_block) calls
    /// are complete. It updates the on-disk header to reflect any changes made
    /// via [`header`](Self::header) (such as updated `dmin`/`dmax`/`dmean`/`rms`
    /// statistics after calling [`update_header_stats`](Self::update_header_stats)).
    ///
    /// # Errors
    /// Returns [`Error::Io`] if seeking or writing fails.
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
    let nx = header.nx.max(0) as usize;
    let ny = header.ny.max(0) as usize;
    let nz = header.nz.max(0) as usize;
    let (dmin, dmax, dmean, rms) =
        crate::engine::stats::compute_stats(bytes, mode, endian, nx, ny * nz)?;
    header.dmin = dmin;
    header.dmax = dmax;
    header.dmean = dmean;
    header.rms = rms;
    Ok(())
}

// ============================================================================
// MmapWriter
// ============================================================================

/// Memory-mapped MRC file writer.
///
/// Writes data directly into a memory-mapped region, letting the OS handle
/// paging and flushing. This is efficient for large files and random-access
/// modifications, but requires the `mmap` feature.
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
///         .finish_mmap()?;
///
///     writer.write_block(&VoxelBlock::new(
///         [0, 0, 0], [512, 512, 1],
///         vec![0.0f32; 512 * 512],
///     )?)?;
///     Ok(())
/// }
/// ```
///
/// For most use cases, prefer creating via [`WriterBuilder::finish_mmap`](crate::WriterBuilder::finish_mmap).
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
    pub(crate) fn create<P: AsRef<std::path::Path>>(
        path: P,
        mut header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
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

        // Write extended header (provided bytes or zeros).
        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            if ext_header.len() >= ext_size {
                file.write_all(&ext_header[..ext_size])?;
            } else {
                file.write_all(ext_header)?;
                let remaining = ext_size - ext_header.len();
                let zeros = vec![0u8; remaining];
                file.write_all(&zeros)?;
            }
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

    /// Volume dimensions for this writer.
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode for this writer.
    ///
    /// Falls back to [`Mode::Float32`] if the header mode value is not recognised.
    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    /// Reference to the current header.
    ///
    /// Modify header fields before calling [`finalize`](Self::finalize) to
    /// change what gets written to disk.
    pub fn header(&self) -> &Header {
        &self.header
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
        write_u8_block_body!(self, block)
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

        let file_endian = self.header.detect_endian();
        crate::io::reader_common::encode_block_to_buf(
            block,
            self.shape,
            self.bytes_per_voxel,
            file_endian,
            self.data_offset,
            &mut self.mmap,
        )
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
            .try_for_each(|(chunk_idx, chunk)| {
                let start_offset = base_offset + chunk_idx * chunk_size * self.bytes_per_voxel;
                let ptr = (mmap_ptr + start_offset) as *mut u8;
                let dst = unsafe {
                    core::slice::from_raw_parts_mut(ptr, chunk.len() * self.bytes_per_voxel)
                };

                encode_slice(chunk, dst, file_endian)
            })?;

        Ok(())
    }

    /// Write an `f32` block to a Float16 file.
    #[cfg(feature = "f16")]
    pub fn write_f16_from_f32(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        write_f16_from_f32_body!(self, block)
    }

    /// Write a block of `u8` data (0–15 per voxel) by packing to 4-bit (Mode 101).
    pub fn write_u4_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        write_u4_block_body!(self, block)
    }

    /// Write raw packed bytes at the given block offset.
    fn write_block_bytes(
        &mut self,
        packed: &[u8],
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<(), Error> {
        crate::io::reader_common::write_block_bytes(
            packed,
            self.shape,
            offset,
            shape,
            self.data_offset,
            &mut self.mmap,
        )
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
        if end > self.mmap.len() {
            return Err(Error::BoundsError);
        }
        update_header_stats_from_bytes(&mut self.header, &self.mmap[self.data_offset..end])?;
        Ok(())
    }
}

#[cfg(feature = "mmap")]
impl MmapWriter {
    /// Finalize the memory-mapped MRC file by writing the header.
    ///
    /// This updates the first 1024 bytes of the memory map with the current
    /// header and flushes the mapping to disk. Must be called after all
    /// [`write_block`](MmapWriter::write_block) calls.
    ///
    /// # Errors
    /// Returns [`Error::Io`] if the flush fails.
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
///
/// This trait is `#[doc(hidden)]` — users should use the concrete type
/// aliases [`GzipWriter`](crate::GzipWriter) and [`Bzip2Writer`](crate::Bzip2Writer).
#[doc(hidden)]
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
    ext_header: Vec<u8>,
    path: std::path::PathBuf,
    bytes_per_voxel: usize,
    shape: VolumeShape,
    _marker: std::marker::PhantomData<C>,
}

impl<C: Compressor> CompressedWriter<C> {
    pub(crate) fn create<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        let mut header = header;
        header.set_file_endian(FileEndian::LittleEndian);

        // Sync nsymbt with provided ext_header if non-empty.
        if !ext_header.is_empty() {
            header.nsymbt = ext_header.len() as i32;
        }
        header.validate_detailed()?;

        let ext_size = header.nsymbt as usize;
        let ext_header_stored = if ext_header.len() >= ext_size {
            ext_header[..ext_size].to_vec()
        } else {
            let mut v = ext_header.to_vec();
            v.resize(ext_size, 0);
            v
        };

        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let data = vec![0u8; data_size];

        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let bytes_per_voxel = mode.byte_size();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            header,
            data,
            ext_header: ext_header_stored,
            path: path.as_ref().to_path_buf(),
            bytes_per_voxel,
            shape,
            _marker: std::marker::PhantomData,
        })
    }

    /// Volume dimensions for this writer.
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode for this writer.
    ///
    /// Falls back to [`Mode::Float32`] if the header mode value is not recognised.
    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    /// Reference to the current header.
    ///
    /// Modify header fields before calling [`finalize`](Self::finalize) to
    /// change what gets written to disk.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Write a block of voxels to the in-memory buffer.
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

        let file_endian = self.header.detect_endian();
        // NOTE: self.data contains only the voxel data (not the header),
        // so data_offset is 0.
        crate::io::reader_common::encode_block_to_buf(
            block,
            self.shape,
            self.bytes_per_voxel,
            file_endian,
            0,
            &mut self.data,
        )
    }

    /// Write a block of `u8` data by automatically widening to `u16` (Mode 6).
    ///
    /// The file must have been created with [`Mode::Uint16`]. Each `u8` voxel
    /// is widened to `u16` before writing, matching Python `mrcfile`'s
    /// auto-conversion behaviour for `np.uint8` data.
    pub fn write_u8_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        write_u8_block_body!(self, block)
    }

    /// Write an `f32` block to a Float16 file.
    ///
    /// This is a convenience method for the common case of writing f32 data
    /// to a half-precision MRC file.
    #[cfg(feature = "f16")]
    pub fn write_f16_from_f32(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        write_f16_from_f32_body!(self, block)
    }

    /// Write a block of `u8` data (0–15 per voxel) by packing to 4-bit (Mode 101).
    pub fn write_u4_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        write_u4_block_body!(self, block)
    }

    /// Write raw packed bytes at the given block offset.
    fn write_block_bytes(
        &mut self,
        packed: &[u8],
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<(), Error> {
        crate::io::reader_common::write_block_bytes(
            packed,
            self.shape,
            offset,
            shape,
            0, // data starts at offset 0 within self.data
            &mut self.data,
        )
    }

    /// Scan the written data block and update `dmin`, `dmax`, `dmean` and `rms`
    /// in the header to match the actual file contents.
    ///
    /// Unlike [`Writer::update_header_stats`], this does not need to read from
    /// disk because the data is already accessible in memory.
    pub fn update_header_stats(&mut self) -> Result<(), Error> {
        update_header_stats_from_bytes(&mut self.header, &self.data)
    }

    /// Finalize the compressed MRC file by assembling, compressing and writing to disk.
    ///
    /// Assembles the full MRC file (header + extended header + voxel data),
    /// compresses it via the backend compressor, and writes the result to the
    /// output path. After this call the writer is consumed.
    ///
    /// # Errors
    /// Returns [`Error::Io`] if the file cannot be written.
    pub fn finalize(self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);

        let ext_size = self.header.nsymbt as usize;
        let mut file_bytes = Vec::with_capacity(1024 + ext_size + self.data.len());
        file_bytes.extend_from_slice(&header_bytes);
        if ext_size > 0 {
            file_bytes.extend_from_slice(&self.ext_header);
        }
        file_bytes.extend_from_slice(&self.data);

        let compressed = C::compress(&file_bytes)?;
        std::fs::write(&self.path, compressed)?;
        Ok(())
    }
}
