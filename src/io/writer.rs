//! MRC file writer with block-based API.
//!
//! Provides [`Writer`], a unified writer supporting file, mmap, gzip, and bzip2 output.
//! (memory-mapped, requires `mmap`), and [`CompressedWriter`] (gzip/bzip2 backend).
//! Use [`WriterBuilder`] or the [`create`](crate::create) convenience function
//! to construct a writer.
//!
//! # Typical write lifecycle
//!
//! 1. Build a writer with [`create`](crate::create) or [`WriterBuilder::new`].
//! 2. Write [`VoxelBlock`](crate::VoxelBlock)s with [`write_block`](Writer::write_block)
//!    or [`write_block_as`](Writer::write_block_as) for automatic type conversion.
//! 3. Optionally call [`update_header_stats`](Writer::update_header_stats) to
//!    fill header density statistics.
//! 4. Call [`finalize`](Writer::finalize) to rewrite the header with final metadata.

macro_rules! write_u4_block_body {
    ($self:ident, $block:ident) => {{
        if $self.mode() != Mode::Packed4Bit {
            return Err(Error::ModeMismatch {
                file_mode: $self.mode(),
                requested_mode: Mode::Packed4Bit,
                offset: None,
            });
        }
        if !$self.shape().contains_block($block.offset, $block.shape) {
            return Err(Error::bounds_err());
        }
        for &v in &$block.data {
            if v > 15 {
                return Err(crate::Error::ValueOutOfRange {
                    value: v as u64,
                    max: 15,
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

// Helper: write block with automatic type conversion to the file's mode.
// Dispatches on self.mode() at runtime.
macro_rules! write_block_as_body {
    ($self:ident, $block:ident) => {{
        if !$self.shape().contains_block($block.offset, $block.shape) {
            return Err(Error::bounds_err());
        }
        match $self.mode() {
            Mode::Int8 => {
                let data = crate::engine::convert::convert_f32_slice_to_i8(&$block.data);
                $self.write_block_data::<i8>($block.offset, $block.shape, &data)
            }
            Mode::Int16 => {
                let data = crate::engine::convert::convert_f32_slice_to_i16(&$block.data);
                $self.write_block_data::<i16>($block.offset, $block.shape, &data)
            }
            Mode::Uint16 => {
                let data = crate::engine::convert::convert_f32_slice_to_u16(&$block.data);
                $self.write_block_data::<u16>($block.offset, $block.shape, &data)
            }
            #[cfg(feature = "f16")]
            Mode::Float16 => {
                let data = crate::engine::convert::convert_f32_slice_to_f16(&$block.data);
                $self.write_block_data::<crate::f16>($block.offset, $block.shape, &data)
            }
            Mode::Float32 => {
                // f32 → Float32: pass through directly, no clone needed
                $self.write_block_data::<f32>($block.offset, $block.shape, &$block.data)
            }
            // Complex modes and Packed4Bit are not convertible from real f32 data.
            // Use write_block::<T>() with the matching complex type directly.
            _ => Err(Error::UnsupportedMode),
        }
    }};
}

/// Compression level for compressed MRC writers.
///
/// Controls the trade-off between compression speed and file size.
/// Used by compressed writers created via [`WriterBuilder::finish_gzip`]
/// and [`WriterBuilder::finish_bzip2`].
///
/// # Example
/// ```no_run
/// use mrc::{Compression, create};
///
/// let mut writer = create("output.mrc.gz")
///     .shape([256, 256, 128])
///     .mode::<f32>()
///     .compression(Compression::Best)
///     .finish_gzip()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Compression {
    /// No compression (gzip/bzip2 level 0 — stored format, minimal CPU).
    None,
    /// Fast compression (minimal CPU, larger output).
    Fast,
    /// Balanced compression (default).
    #[default]
    Balanced,
    /// Maximum compression (more CPU, smaller output).
    Best,
}

impl Compression {
    /// Map to flate2 compression level.
    #[cfg(feature = "gzip")]
    pub(crate) fn to_flate2(self) -> flate2::Compression {
        match self {
            Compression::None => flate2::Compression::none(),
            Compression::Fast => flate2::Compression::fast(),
            Compression::Balanced => flate2::Compression::default(),
            Compression::Best => flate2::Compression::best(),
        }
    }

    /// Map to bzip2 compression level.
    #[cfg(feature = "bzip2")]
    pub(crate) fn to_bzip2(self) -> bzip2::Compression {
        match self {
            Compression::None | Compression::Fast => bzip2::Compression::fast(),
            Compression::Balanced => bzip2::Compression::default(),
            Compression::Best => bzip2::Compression::best(),
        }
    }
}

use crate::engine::block::{VolumeShape, VoxelBlock};
#[cfg(feature = "parallel")]
use crate::engine::codec::encode_block_parallel;
use crate::engine::codec::encode_slice;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// How the writer persists voxel data.
enum DataSink {
    /// Standard file I/O or generic I/O target.
    File(Box<dyn ReadWriteSeek + 'static>),
    /// Memory-mapped file.
    #[cfg(feature = "mmap")]
    Mmap(memmap2::MmapMut),
    /// Buffered in memory; compressed and written to disk on finalize.
    Compressed {
        buf: Vec<u8>,
        path: std::path::PathBuf,
        compression: Compression,
        is_gzip: bool,
    },
}

/// Trait alias for types that support read, write, and seek simultaneously.
///
/// Required by [`Writer`] which needs random-access read-back for
/// [`update_header_stats`](Writer::update_header_stats) and must seek back
/// to rewrite the header on [`finalize`](Writer::finalize).
pub trait ReadWriteSeek: Read + Write + Seek {}
impl<T: Read + Write + Seek> ReadWriteSeek for T {}

macro_rules! builder_setters {
    () => {
        /// Set the volume dimensions.
        ///
        /// Also synchronises `mx`, `my`, `mz` to match `nx`, `ny`, `nz`.
        #[must_use]
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
        #[must_use]
        pub fn mode<T: Voxel>(mut self) -> Self {
            self.header.mode = T::MODE.as_i32();
            self
        }

        /// Set the MRC mode by raw integer value (for modes without a [`Voxel`] impl).
        ///
        /// This is primarily useful for [`Mode::Packed4Bit`] (mode 101) which does not
        /// implement `Voxel`.  Invalid mode constants are caught by header validation
        /// at `finish()` time.
        #[must_use]
        pub fn mode_raw(mut self, mode: i32) -> Self {
            self.header.mode = mode;
            self
        }

        /// Set the cell dimensions in Angstroms.
        #[must_use]
        pub fn cell_lengths(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
            self.header.xlen = xlen;
            self.header.ylen = ylen;
            self.header.zlen = zlen;
            self
        }

        /// Set the space group number.
        #[must_use]
        pub fn ispg(mut self, ispg: i32) -> Self {
            self.header.ispg = ispg;
            self
        }

        /// Configure as a volume stack with the given sub-volume thickness.
        ///
        /// Shorthand for calling [`ispg(401)`](Self::ispg) and
        /// [`sampling([nx, ny, mz])`](Self::sampling).  `nz` must be divisible
        /// by `mz` for the header to validate.
        ///
        /// Call **after** [`shape`](Self::shape) so that `nx` and `ny` are
        /// already set and only `mz` is overridden.
        #[must_use]
        pub fn set_volume_stack(mut self, mz: i32) -> Self {
            self.header.set_volume_stack(mz);
            self
        }

        /// Set the extended header type (4-byte ASCII identifier).
        #[must_use]
        pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
            self.header.set_exttyp(exttyp);
            self
        }

        /// Set the extended header size in bytes.
        #[must_use]
        pub fn nsymbt(mut self, nsymbt: i32) -> Self {
            self.header.nsymbt = nsymbt;
            self
        }

        /// Set the origin coordinates.
        #[must_use]
        pub fn origin(mut self, origin: [f32; 3]) -> Self {
            self.header.origin = origin;
            self
        }

        /// Set the cell angles in degrees (alpha, beta, gamma).
        #[must_use]
        pub fn cell_angles(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
            self.header.alpha = alpha;
            self.header.beta = beta;
            self.header.gamma = gamma;
            self
        }

        /// Set the sub-volume origin in pixels (`nxstart`, `nystart`, `nzstart`).
        #[must_use]
        pub fn nstart(mut self, nstart: [i32; 3]) -> Self {
            self.header.nxstart = nstart[0];
            self.header.nystart = nstart[1];
            self.header.nzstart = nstart[2];
            self
        }

        /// Set the sampling rates (`mx`, `my`, `mz`) independently of the volume
        /// dimensions.
        ///
        /// By default [`shape`](Self::shape) syncs `mx`, `my`, `mz` to `nx`, `ny`,
        /// `nz`.  Use this method to override them when the cell sampling differs
        /// from the pixel dimensions.
        #[must_use]
        pub fn sampling(mut self, sampling: [i32; 3]) -> Self {
            self.header.mx = sampling[0];
            self.header.my = sampling[1];
            self.header.mz = sampling[2];
            self
        }

        /// Set the axis mapping (`mapc`, `mapr`, `maps`) — a permutation of
        /// `1` (X), `2` (Y), `3` (Z) that defines which axis is column, row,
        /// and section.
        #[must_use]
        pub fn axis_mapping(mut self, mapping: [i32; 3]) -> Self {
            self.header.mapc = mapping[0];
            self.header.mapr = mapping[1];
            self.header.maps = mapping[2];
            self
        }

        /// Append a text label (up to 10 labels, FIFO eviction when full).
        #[must_use]
        pub fn add_label(mut self, text: &str) -> Self {
            self.header.add_label(text);
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
    compression: Compression,
}

impl WriterBuilder {
    /// Create a new builder with default header values.
    #[must_use]
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            header: Header::new(),
            ext_header: Vec::new(),
            compression: Compression::Balanced,
        }
    }

    /// Set the compression level for compressed writers.
    ///
    /// Affects [`finish_gzip`](Self::finish_gzip) and
    /// [`finish_bzip2`](Self::finish_bzip2). Has no effect on
    /// [`finish`](Self::finish) (plain) or [`finish_mmap`](Self::finish_mmap).
    ///
    /// Default: [`Compression::Balanced`].
    #[must_use]
    pub fn compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }

    builder_setters!();

    /// Set the extended header bytes.
    ///
    /// When provided, `nsymbt` is automatically updated to match the byte
    /// length. Pass an empty `Vec` (or omit) to write zeros for the extended
    /// header region.
    #[must_use]
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
    /// Equivalent to [`finish`](Self::finish) but uses memory-mapped output
    /// (requires the `mmap` feature).
    #[cfg(feature = "mmap")]
    pub fn finish_mmap(self) -> Result<Writer, Error> {
        Writer::create_mmap(self.path, self.header, &self.ext_header)
    }

    /// Build a gzip-compressed writer.
    ///
    /// Because gzip does not support random access, the entire file is buffered
    /// in memory and compressed only on finalize.
    /// For large volumes consider using [`finish`](Self::finish) instead.
    #[cfg(feature = "gzip")]
    pub fn finish_gzip(self) -> Result<Writer, Error> {
        Writer::create_compressed(
            self.path,
            self.header,
            &self.ext_header,
            self.compression,
            true,
        )
    }

    /// Build a bzip2-compressed writer.
    ///
    /// Because bzip2 does not support random access, the entire file is buffered
    /// in memory and compressed only on finalize.
    /// For large volumes consider using [`finish`](Self::finish) instead.
    #[cfg(feature = "bzip2")]
    pub fn finish_bzip2(self) -> Result<Writer, Error> {
        Writer::create_compressed(
            self.path,
            self.header,
            &self.ext_header,
            self.compression,
            false,
        )
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
///
/// To write to an in-memory buffer instead of a file, use
/// [`from_writer`](Self::from_writer) with a [`std::io::Cursor`]:
///
/// ```no_run
/// use mrc::{Header, Writer};
/// use std::io::Cursor;
///
/// # fn main() -> Result<(), mrc::Error> {
/// let buffer: Vec<u8> = Vec::new();
/// let header = Header::new();
/// let mut writer = Writer::from_writer(Cursor::new(buffer), header, &[])?;
/// // ... write blocks, then finalize
/// writer.finalize()?;
/// # Ok(()) }
/// ```
pub struct Writer {
    header: Header,
    data_offset: u64,
    bytes_per_voxel: usize,
    mode: Mode,
    shape: VolumeShape,
    sink: DataSink,
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer")
            .field("header", &self.header)
            .field("data_offset", &self.data_offset)
            .field("bytes_per_voxel", &self.bytes_per_voxel)
            .field("mode", &self.mode)
            .field("shape", &self.shape)
            .finish()
    }
}

impl Writer {
    /// Create a writer that writes to an arbitrary [`std::io::Read`] +
    /// [`std::io::Write`] + [`std::io::Seek`] target.
    ///
    /// This enables writing directly to in-memory buffers:
    ///
    /// ```no_run
    /// use mrc::{Header, Writer};
    /// use std::io::Cursor;
    ///
    /// # fn main() -> Result<(), mrc::Error> {
    /// let header = Header::new();
    /// let mut writer = Writer::from_writer(Cursor::new(Vec::new()), header, &[])?;
    /// // ... write blocks, then finalize
    /// writer.finalize()?;
    /// # Ok(()) }
    /// ```
    pub fn from_writer<W: Read + Write + Seek + 'static>(
        writer: W,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        // New files are always little-endian per crate policy
        Self::_create(Box::new(writer), header, ext_header)
    }

    /// Create a memory-mapped writer from a [`Header`] directly.
    ///
    /// Like [`from_writer`](Self::from_writer) but uses a memory-mapped file
    /// instead of a file handle. Requires the `mmap` feature.
    ///
    /// The file is truncated to the exact size needed for the header, extended
    /// header, and voxel data.
    #[cfg(feature = "mmap")]
    pub fn from_writer_mmap<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        Self::create_mmap(path, header, ext_header)
    }

    /// Create a gzip-compressed writer from a [`Header`] directly.
    ///
    /// Like [`from_writer`](Self::from_writer) but buffers the entire file in
    /// memory and gzip-compresses it on [`finalize`](Self::finalize).
    /// Requires the `gzip` feature.
    #[cfg(feature = "gzip")]
    pub fn from_writer_gzip<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
        compression: Compression,
    ) -> Result<Self, Error> {
        Self::create_compressed(path, header, ext_header, compression, true)
    }

    /// Create a bzip2-compressed writer from a [`Header`] directly.
    ///
    /// Like [`from_writer`](Self::from_writer) but buffers the entire file in
    /// memory and bzip2-compresses it on [`finalize`](Self::finalize).
    /// Requires the `bzip2` feature.
    #[cfg(feature = "bzip2")]
    pub fn from_writer_bzip2<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
        compression: Compression,
    ) -> Result<Self, Error> {
        Self::create_compressed(path, header, ext_header, compression, false)
    }

    pub(crate) fn create<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Self::_create(Box::new(file), header, ext_header)
    }

    fn _create(
        mut io: Box<dyn ReadWriteSeek + 'static>,
        mut header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        // New files are always little-endian per crate policy
        header.set_file_endian(FileEndian::LittleEndian);

        header.validate_detailed()?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        io.write_all(&header_bytes)?;

        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            if ext_header.len() >= ext_size {
                io.write_all(&ext_header[..ext_size])?;
            } else {
                // Pad with zeros if provided bytes are shorter than nsymbt
                io.write_all(ext_header)?;
                let remaining = ext_size - ext_header.len();
                let zeros = vec![0u8; remaining];
                io.write_all(&zeros)?;
            }
        }

        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Int16Complex {
            tracing::warn!(
                "Mode 3 (Int16Complex) is obsolete and should not be used for writing new files."
            );
        }
        let bytes_per_voxel = mode.byte_size();

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            header,
            data_offset,
            bytes_per_voxel,
            mode,
            shape,
            sink: DataSink::File(io),
        })
    }

    /// Create a memory-mapped writer.
    #[cfg(feature = "mmap")]
    pub(crate) fn create_mmap<P: AsRef<std::path::Path>>(
        path: P,
        mut header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        header.set_file_endian(FileEndian::LittleEndian);
        header.validate_detailed()?;
        let total_size = header
            .data_offset()
            .checked_add(header.data_size().ok_or(Error::InvalidHeader)?)
            .ok_or(Error::InvalidHeader)?;
        let mmap = {
            use std::fs::OpenOptions;
            use std::io::Write;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;
            file.set_len(total_size as u64)?;
            let mut hb = [0u8; 1024];
            header.encode_to_bytes(&mut hb);
            (&file).write_all(&hb)?;
            let ext_size = header.nsymbt as usize;
            if ext_size > 0 {
                if ext_header.len() >= ext_size {
                    (&file).write_all(&ext_header[..ext_size])?;
                } else {
                    (&file).write_all(ext_header)?;
                    (&file).write_all(&vec![0u8; ext_size - ext_header.len()])?;
                }
            }
            unsafe {
                memmap2::MmapOptions::new()
                    .map_mut(&file)
                    .map_err(|_| Error::Mmap)?
            }
        };

        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Int16Complex {
            tracing::warn!(
                "Mode 3 (Int16Complex) is obsolete and should not be used for writing new files."
            );
        }
        let bytes_per_voxel = mode.byte_size();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);
        Ok(Self {
            header,
            data_offset,
            bytes_per_voxel,
            mode,
            shape,
            sink: DataSink::Mmap(mmap),
        })
    }

    /// Create a compressed writer.
    #[cfg(any(feature = "gzip", feature = "bzip2"))]
    pub(crate) fn create_compressed<P: AsRef<std::path::Path>>(
        path: P,
        mut header: Header,
        ext_header: &[u8],
        compression: Compression,
        is_gzip: bool,
    ) -> Result<Self, Error> {
        header.set_file_endian(FileEndian::LittleEndian);
        if !ext_header.is_empty() {
            header.nsymbt = ext_header.len() as i32;
        }
        header.validate_detailed()?;
        let ext_size = header.nsymbt as usize;
        let ext_stored = if ext_header.len() >= ext_size {
            ext_header[..ext_size].to_vec()
        } else {
            let mut v = ext_header.to_vec();
            v.resize(ext_size, 0);
            v
        };
        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let off = header.data_offset();
        let mut buf = vec![0u8; off + data_size];
        let mut hb = [0u8; 1024];
        header.encode_to_bytes(&mut hb);
        buf[..1024].copy_from_slice(&hb);
        if ext_size > 0 {
            buf[1024..1024 + ext_size].copy_from_slice(&ext_stored);
        }
        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Int16Complex {
            tracing::warn!(
                "Mode 3 (Int16Complex) is obsolete and should not be used for writing new files."
            );
        }
        let bytes_per_voxel = mode.byte_size();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);
        Ok(Self {
            header,
            data_offset,
            bytes_per_voxel,
            mode,
            shape,
            sink: DataSink::Compressed {
                buf,
                path: path.as_ref().to_path_buf(),
                compression,
                is_gzip,
            },
        })
    }

    /// Volume dimensions for this writer.
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode for this writer.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Read-only reference to the current header.
    ///
    /// For mutable access, use [`header_mut`](Self::header_mut).
    /// Modify header fields before calling [`finalize`](Self::finalize) to
    /// change what gets written to disk.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Mutable reference to the current header.
    ///
    /// Allows modifying header fields (e.g. labels, density statistics)
    /// between writing blocks and calling [`finalize`](Self::finalize).
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
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
                offset: None,
            });
        }
        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::bounds_err());
        }
        self.write_block_data::<T>(block.offset, block.shape, &block.data)
    }

    /// Core write implementation: encode and persist typed voxel data.
    ///
    /// Bounds and mode checks must be performed by the caller beforehand.
    fn write_block_data<T: Voxel>(
        &mut self,
        offset: [usize; 3],
        shape: [usize; 3],
        data: &[T],
    ) -> Result<(), Error> {
        let file_endian = self.header.detect_endian();

        match &mut self.sink {
            DataSink::File(io) => {
                let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
                let [ox, oy, oz] = offset;
                let [sx, sy, sz] = shape;
                let b = self.bytes_per_voxel;

                if ox == 0 && sx == nx && oy == 0 && sy == ny {
                    let linear = (ox as u64)
                        + (oy as u64) * (nx as u64)
                        + (oz as u64) * (nx as u64) * (ny as u64);
                    let start_offset = self.data_offset + linear * (b as u64);
                    let byte_len = (sx as u64) * (sy as u64) * (sz as u64) * (b as u64);
                    let byte_len_usize = byte_len.try_into().map_err(|_| Error::bounds_err())?;
                    let mut buffer = vec![0u8; byte_len_usize];
                    encode_slice(data, &mut buffer, file_endian)?;
                    io.seek(SeekFrom::Start(start_offset))?;
                    io.write_all(&buffer)?;
                    return Ok(());
                }

                let mut row_bytes = vec![0u8; sx * b];
                for z in 0..sz {
                    for y in 0..sy {
                        let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
                        let file_offset = self.data_offset + (file_linear as u64) * (b as u64);
                        let block_idx = y * sx + z * sx * sy;
                        if block_idx + sx > data.len() {
                            return Err(Error::bounds_err());
                        }
                        let row_values = &data[block_idx..block_idx + sx];
                        encode_slice(row_values, &mut row_bytes, file_endian)?;
                        io.seek(SeekFrom::Start(file_offset))?;
                        io.write_all(&row_bytes)?;
                    }
                }
                Ok(())
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => {
                let block = VoxelBlock {
                    offset,
                    shape,
                    data: data.to_vec(),
                };
                crate::io::reader_common::encode_block_to_buf(
                    &block,
                    self.shape,
                    self.bytes_per_voxel,
                    file_endian,
                    self.data_offset as usize,
                    mmap,
                )
            }
            DataSink::Compressed { buf, .. } => {
                let block = VoxelBlock {
                    offset,
                    shape,
                    data: data.to_vec(),
                };
                crate::io::reader_common::encode_block_to_buf(
                    &block,
                    self.shape,
                    self.bytes_per_voxel,
                    file_endian,
                    self.data_offset as usize,
                    buf,
                )
            }
        }
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
                offset: None,
            });
        }
        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::bounds_err());
        }
        let widened = crate::engine::convert::convert_u8_slice_to_u16(&block.data);
        self.write_block_data::<u16>(block.offset, block.shape, &widened)
    }

    /// Write a block with automatic type conversion to the file's mode.
    ///
    /// The caller provides data as `f32` and it is converted to the file's
    /// on-disk mode. Supported conversions:
    ///
    /// | File mode | Conversion |
    /// |-----------|------------|
    /// | [`Int8`](Mode::Int8) | `f32` → `i8` (clamped, SIMD) |
    /// | [`Int16`](Mode::Int16) | `f32` → `i16` (clamped, SIMD) |
    /// | [`Uint16`](Mode::Uint16) | `f32` → `u16` (clamped, SIMD) |
    /// | [`Float32`](Mode::Float32) | `f32` → `f32` (pass-through) |
    /// | [`Float16`](Mode::Float16) | `f32` → `f16` (SIMD, requires `f16` feature) |
    ///
    /// Complex modes and Packed4Bit are not convertible from real f32 data.
    /// Use [`write_block`](Writer::write_block) with the matching complex type instead.
    pub fn write_block_as(&mut self, block: &VoxelBlock<f32>) -> Result<(), Error> {
        write_block_as_body!(self, block)
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
                offset: None,
            });
        }
        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::bounds_err());
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, _sz] = block.shape;

        // Parallel fast path only works for full XY slabs (contiguous in file).
        if ox != 0 || sx != nx || oy != 0 || sy != ny {
            return self.write_block(block);
        }

        // Only file-backed writers support parallel seeks.
        let DataSink::File(io) = &mut self.sink else {
            return self.write_block(block);
        };

        let chunk_size = 1024 * 1024;
        let linear =
            (ox as u64) + (oy as u64) * (nx as u64) + (oz as u64) * (nx as u64) * (ny as u64);
        let base_offset = self.data_offset + linear * (self.bytes_per_voxel as u64);
        let file_endian = self.header.detect_endian();
        let encoded_chunks = encode_block_parallel(&block.data, chunk_size, file_endian);

        for (chunk_idx, encoded) in encoded_chunks {
            let offset = base_offset
                + (chunk_idx as u64) * (chunk_size as u64) * (self.bytes_per_voxel as u64);
            io.seek(SeekFrom::Start(offset))?;
            io.write_all(&encoded)?;
        }
        Ok(())
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
    /// Only full-row writes (`ox == 0`) are supported; sub-XY blocks with
    /// non-zero X-offset return [`Error::BoundsError`].
    ///
    /// Internal helper used by [`write_u4_block`](Self::write_u4_block).
    fn write_block_bytes(
        &mut self,
        packed: &[u8],
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<(), Error> {
        match &mut self.sink {
            DataSink::File(io) => {
                let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
                let [ox, oy, oz] = offset;
                let [sx, sy, sz] = shape;
                let file_row_bytes = nx.div_ceil(2);
                let block_row_bytes = sx.div_ceil(2);
                if ox != 0 {
                    return Err(Error::bounds_err());
                }
                if sx == nx && oy == 0 && sy == ny {
                    let slice_bytes = ny * file_row_bytes;
                    let start_offset = (self.data_offset as usize) + oz * slice_bytes;
                    let byte_len = sz * slice_bytes;
                    io.seek(SeekFrom::Start(start_offset as u64))?;
                    io.write_all(&packed[..byte_len])?;
                    return Ok(());
                }
                for z in 0..sz {
                    for y in 0..sy {
                        let vol_row = (oz + z) * ny + (oy + y);
                        let file_offset = (self.data_offset as usize) + vol_row * file_row_bytes;
                        let packed_start = (y + z * sy) * block_row_bytes;
                        let packed_end = packed_start + block_row_bytes;
                        if packed_end > packed.len() {
                            return Err(Error::bounds_err());
                        }
                        io.seek(SeekFrom::Start(file_offset as u64))?;
                        io.write_all(&packed[packed_start..packed_end])?;
                    }
                }
                Ok(())
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => crate::io::reader_common::write_block_bytes(
                packed,
                self.shape,
                offset,
                shape,
                self.data_offset as usize,
                mmap,
            ),
            DataSink::Compressed { buf, .. } => crate::io::reader_common::write_block_bytes(
                packed,
                self.shape,
                offset,
                shape,
                self.data_offset as usize,
                buf,
            ),
        }
    }

    /// Finalize the MRC file by rewriting the header.
    pub fn finalize(&mut self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);

        match &mut self.sink {
            DataSink::File(io) => {
                io.seek(SeekFrom::Start(0))?;
                io.write_all(&header_bytes)?;
                Ok(())
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => {
                mmap[0..1024].copy_from_slice(&header_bytes);
                mmap.flush()?;
                Ok(())
            }
            DataSink::Compressed {
                buf,
                path,
                compression,
                is_gzip,
            } => {
                buf[..1024].copy_from_slice(&header_bytes);
                let compressed = compress_data(buf, *compression, *is_gzip)?;
                std::fs::write(path, compressed)?;
                Ok(())
            }
        }
    }

    /// Scan the written data block and update header statistics.
    pub fn update_header_stats(&mut self) -> Result<(), Error> {
        let (data_offset, data_size) = {
            let ds = self.header.data_size().ok_or(Error::InvalidHeader)?;
            (self.header.data_offset(), ds)
        };
        match &mut self.sink {
            DataSink::File(io) => {
                let mut buf = vec![0u8; data_size];
                io.seek(SeekFrom::Start(self.data_offset))?;
                io.read_exact(&mut buf)?;
                update_header_stats_from_bytes(&mut self.header, &buf)
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => {
                let end = self.data_offset as usize + data_size;
                if end > mmap.len() {
                    return Err(Error::bounds_err());
                }
                update_header_stats_from_bytes(
                    &mut self.header,
                    &mmap[self.data_offset as usize..end],
                )
            }
            DataSink::Compressed { buf, .. } => {
                let end = data_offset + data_size;
                if end > buf.len() {
                    return Err(Error::bounds_err());
                }
                update_header_stats_from_bytes(&mut self.header, &buf[data_offset..end])
            }
        }
    }
}

// ============================================================================
// Stats helpers and compression
// ============================================================================

/// Compress MRC data using the appropriate algorithm based on compression level.
#[cfg(any(feature = "gzip", feature = "bzip2"))]
fn compress_data(data: &[u8], compression: Compression, is_gzip: bool) -> Result<Vec<u8>, Error> {
    #[cfg(feature = "gzip")]
    if is_gzip {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), compression.to_flate2());
        std::io::Write::write_all(&mut encoder, data)?;
        return Ok(encoder.finish()?);
    }
    #[cfg(feature = "bzip2")]
    if !is_gzip {
        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), compression.to_bzip2());
        std::io::Write::write_all(&mut encoder, data)?;
        return Ok(encoder.finish()?);
    }
    Err(Error::UnsupportedMode)
}

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
