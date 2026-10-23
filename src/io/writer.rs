//! MRC file writer with block-based API.
//!
//! Provides [`Writer`], a unified writer supporting file, mmap, gzip, and bzip2 output.
//! Use [`WriterBuilder`] or the [`create`](crate::create) convenience function
//! to construct a writer.
//!
//! # Typical write lifecycle
//!
//! 1. Build a writer with [`create`](crate::create) or [`WriterBuilder::new`].
//! 2. Write [`DataBlock`](crate::DataBlock)s with [`write_data_block`](Writer::write_data_block)
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
                offset: Some($block.offset),
            });
        }
        $self.validate_voxel_block($block.offset, $block.shape, $block.data.len())?;
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
        let endian = $self.header().detect_endian();
        match $self.mode() {
            Mode::Int8 => {
                let bytes = crate::engine::convert::convert_f32_to_i8_bytes(&$block.data, endian);
                $self.write_encoded_data(&bytes, $block.offset, $block.shape)
            }
            Mode::Int16 => {
                let bytes = crate::engine::convert::convert_f32_to_i16_bytes(&$block.data, endian);
                $self.write_encoded_data(&bytes, $block.offset, $block.shape)
            }
            Mode::Uint16 => {
                let bytes = crate::engine::convert::convert_f32_to_u16_bytes(&$block.data, endian);
                $self.write_encoded_data(&bytes, $block.offset, $block.shape)
            }
            #[cfg(feature = "f16")]
            Mode::Float16 => {
                let bytes = crate::engine::convert::convert_f32_to_f16_bytes(&$block.data, endian);
                $self.write_encoded_data(&bytes, $block.offset, $block.shape)
            }
            Mode::Float32 => {
                let [sx, sy, _sz] = $block.shape;
                let [ox, oy, _oz] = $block.offset;
                let [nx, ny, _nz] = [$self.shape.nx, $self.shape.ny, $self.shape.nz];
                // For native-endian full-XY slabs: skip encoding entirely.
                // The f32 bytes are already in file-native byte order.
                if endian == FileEndian::native() && ox == 0 && oy == 0 && sx == nx && sy == ny {
                    let data_slice = $block.data.as_slice();
                    // SAFETY: f32 has no interior padding and every bit pattern
                    // is a valid u8. The pointer and length are derived from a
                    // valid slice, so `from_raw_parts` is sound.
                    let bytes = unsafe {
                        core::slice::from_raw_parts(
                            data_slice.as_ptr().cast::<u8>(),
                            core::mem::size_of_val(data_slice),
                        )
                    };
                    return $self.write_encoded_data(bytes, $block.offset, $block.shape);
                }
                let b = $self.bytes_per_voxel as u64;
                let byte_len = (sx as u64) * (sy as u64) * ($block.shape[2] as u64) * b;
                let byte_len_usize = byte_len.try_into().map_err(|_| Error::bounds_err())?;
                let mut buffer = vec![0u8; byte_len_usize];
                crate::engine::codec::encode_slice(&$block.data, &mut buffer, endian)?;
                $self.write_encoded_data(&buffer, $block.offset, $block.shape)
            }
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
/// use mrc::{CompressionLevel, create};
///
/// let mut writer = create("output.mrc.gz")
///     .shape([256, 256, 128])
///     .mode::<f32>()
///     .compression(CompressionLevel::Best)
///     .finish_gzip()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CompressionLevel {
    /// No compression (gzip level 0, stored format). bzip2 has no stored mode,
    /// so this maps to the fastest bzip2 level.
    None,
    /// Fast compression (minimal CPU, larger output).
    Fast,
    /// Balanced compression (default).
    #[default]
    Balanced,
    /// Maximum compression (more CPU, smaller output).
    Best,
}

impl CompressionLevel {
    /// Map to flate2 compression level.
    #[cfg(feature = "gzip")]
    pub(crate) fn to_flate2(self) -> flate2::Compression {
        match self {
            CompressionLevel::None => flate2::Compression::none(),
            CompressionLevel::Fast => flate2::Compression::fast(),
            CompressionLevel::Balanced => flate2::Compression::default(),
            CompressionLevel::Best => flate2::Compression::best(),
        }
    }

    /// Map to bzip2 compression level.
    #[cfg(feature = "bzip2")]
    pub(crate) fn to_bzip2(self) -> bzip2::Compression {
        match self {
            CompressionLevel::None | CompressionLevel::Fast => bzip2::Compression::fast(),
            CompressionLevel::Balanced => bzip2::Compression::default(),
            CompressionLevel::Best => bzip2::Compression::best(),
        }
    }
}

use crate::engine::block::{VolumeShape, VoxelBlock};
#[cfg(feature = "f16")]
use crate::engine::codec::EndianCodec;
#[cfg(feature = "parallel")]
use crate::engine::codec::encode_block_parallel;
use crate::engine::codec::encode_slice;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{DataBlock, DataView, Error, Float32Complex, Header, Int16Complex, Mode};
use std::any::TypeId;

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
    #[allow(dead_code)]
    Compressed {
        buf: Vec<u8>,
        path: std::path::PathBuf,
        compression: CompressionLevel,
        is_gzip: bool,
    },
    /// Fully in-memory output, pre-sized to the declared volume.
    ///
    /// Used by [`WriterBuilder::finish_buffer`] and extracted with
    /// [`Writer::into_buffer`].
    Buffer(Vec<u8>),
}

/// Trait alias for types that support read, write, and seek simultaneously.
///
/// Required by [`Writer`] which needs random-access read-back for
/// [`update_header_stats`](Writer::update_header_stats) and must seek back
/// to rewrite the header on [`finalize`](Writer::finalize).
pub trait ReadWriteSeek: Read + Write + Seek {}
impl<T: Read + Write + Seek> ReadWriteSeek for T {}

/// Convert an absolute byte offset + length into a checked index range.
fn checked_byte_range(offset: u64, len: usize, capacity: usize) -> Result<(usize, usize), Error> {
    let start = usize::try_from(offset).map_err(|_| Error::bounds_err())?;
    let end = start.checked_add(len).ok_or_else(Error::bounds_err)?;
    if end > capacity {
        return Err(Error::bounds_err());
    }
    Ok((start, end))
}

macro_rules! builder_setters {
    () => {
        /// Set the volume dimensions.
        ///
        /// Also synchronises `mx`, `my`, `mz` to match `nx`, `ny`, `nz`.
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").shape([256, 256, 128]);
        /// ```
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
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").mode::<f32>();
        /// ```
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
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").mode_raw(2);
        /// ```
        #[must_use]
        pub fn mode_raw(mut self, mode: i32) -> Self {
            self.header.mode = mode;
            self
        }

        /// Set the cell dimensions in Angstroms.
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc")
        ///     .shape([64, 64, 64])
        ///     .cell_lengths(100.0, 100.0, 200.0);
        /// ```
        #[must_use]
        pub fn cell_lengths(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
            self.header.xlen = xlen;
            self.header.ylen = ylen;
            self.header.zlen = zlen;
            self
        }

        /// Set the space group number.
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").ispg(1);
        /// ```
        #[must_use]
        pub fn ispg(mut self, ispg: i32) -> Self {
            self.header.ispg = ispg;
            self
        }

        /// Configure as a volume stack with the given sub-volume thickness.
        ///
        /// Shorthand for calling [`ispg(401)`](Self::ispg) and setting `mz` to
        /// the given value.  `nz` must be divisible by `mz` for the header to
        /// validate.  Call **after** [`shape`](Self::shape) so that `nx`, `ny`,
        /// and `nz` are already set.
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc")
        ///     .shape([64, 64, 64])
        ///     .volume_stack(32);
        /// ```
        #[must_use]
        pub fn volume_stack(mut self, mz: i32) -> Self {
            self.header.set_volume_stack(mz);
            self
        }

        /// Configure as an image stack (`ispg = 0`, `mz = 1`).
        ///
        /// An image stack is a series of 2D images stored in a 3D array.
        ///
        /// Call **after** [`shape`](Self::shape).
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc")
        ///     .shape([64, 64, 10])
        ///     .image_stack();
        /// ```
        #[must_use]
        pub fn image_stack(mut self) -> Self {
            self.header.set_image_stack();
            self
        }

        /// Configure as a single 3D volume (`ispg = 1`, `mz = nz`).
        ///
        /// This is the default for 3D data.  Call only if the file was
        /// previously configured as an image stack or volume stack.
        ///
        /// Call **after** [`shape`](Self::shape).
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc")
        ///     .shape([64, 64, 64])
        ///     .volume();
        /// ```
        #[must_use]
        pub fn volume(mut self) -> Self {
            self.header.set_volume();
            self
        }

        /// Set the extended header type (4-byte ASCII identifier).
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").exttyp(*b"FEI1");
        /// ```
        #[must_use]
        pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
            self.header.set_exttyp(exttyp);
            self
        }

        /// Set the extended header size in bytes.
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").nsymbt(256);
        /// ```
        #[must_use]
        pub fn nsymbt(mut self, nsymbt: i32) -> Self {
            self.header.nsymbt = nsymbt;
            self
        }

        /// Set the origin coordinates.
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").origin([0.0, 0.0, 0.0]);
        /// ```
        #[must_use]
        pub fn origin(mut self, origin: [f32; 3]) -> Self {
            self.header.origin = origin;
            self
        }

        /// Set the cell angles in degrees (alpha, beta, gamma).
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").cell_angles(90.0, 90.0, 90.0);
        /// ```
        #[must_use]
        pub fn cell_angles(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
            self.header.alpha = alpha;
            self.header.beta = beta;
            self.header.gamma = gamma;
            self
        }

        /// Set the sub-volume origin in pixels (`nxstart`, `nystart`, `nzstart`).
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").nstart([0, 0, 0]);
        /// ```
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
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc")
        ///     .shape([64, 64, 64])
        ///     .sampling([32, 32, 32]);
        /// ```
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
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").axis_mapping([1, 2, 3]);
        /// ```
        #[must_use]
        pub fn axis_mapping(mut self, mapping: [i32; 3]) -> Self {
            self.header.mapc = mapping[0];
            self.header.mapr = mapping[1];
            self.header.maps = mapping[2];
            self
        }

        /// Append a text label (up to 10 labels, FIFO eviction when full).
        ///
        /// # Examples
        /// ```
        /// use mrc::create;
        /// let builder = create("output.mrc").add_label("example dataset");
        /// ```
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
/// use mrc::{create, DataBlock, OwnedData};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut writer = create("output.mrc")
///         .shape([512, 512, 256])
///         .mode::<f32>()
///         .finish()?;
///
///     let block = DataBlock::Owned {
///         offset: [0, 0, 0],
///         shape: [512, 512, 1],
///         data: OwnedData::Float32(vec![0.0f32; 512 * 512]),
///     };
///     writer.write_data_block(&block)?;
///     writer.finalize()?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct WriterBuilder {
    path: PathBuf,
    header: Header,
    ext_header: Vec<u8>,
    compression: CompressionLevel,
}

impl WriterBuilder {
    /// Create a new builder with default header values.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::WriterBuilder;
    /// let builder = WriterBuilder::new("output.mrc");
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            header: Header::new(),
            ext_header: Vec::new(),
            compression: CompressionLevel::Balanced,
        }
    }

    /// Set the compression level for compressed writers.
    ///
    /// Affects [`finish_gzip`](Self::finish_gzip) and
    /// [`finish_bzip2`](Self::finish_bzip2). Has no effect on
    /// [`finish`](Self::finish) (plain) or [`finish_mmap`](Self::finish_mmap).
    ///
    /// Default: [`CompressionLevel::Balanced`].
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{WriterBuilder, CompressionLevel};
    /// let mut writer = WriterBuilder::new("output.mrc.gz")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .compression(CompressionLevel::Best)
    ///     .finish_gzip()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn compression(mut self, compression: CompressionLevel) -> Self {
        self.compression = compression;
        self
    }

    builder_setters!();

    /// Set the extended header bytes.
    ///
    /// When provided, `nsymbt` is automatically updated to match the byte
    /// length. Pass an empty `Vec` (or omit) to write zeros for the extended
    /// header region.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::WriterBuilder;
    /// let mut writer = WriterBuilder::new("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .extended_header(vec![0u8; 256])
    ///     .finish()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn extended_header(mut self, bytes: Vec<u8>) -> Self {
        self.header.nsymbt = bytes.len() as i32;
        self.ext_header = bytes;
        self
    }

    /// Consume the builder and create a standard file-backed [`Writer`].
    ///
    /// The file is created (or truncated) and the header + extended header
    /// are written immediately. Voxel data can then be written with
    /// [`write_data_block`](Writer::write_data_block).
    ///
    /// # Errors
    /// Returns [`Error::InvalidHeaderDetailed`] if the header fails validation.
    /// Returns [`Error::Io`] if the file cannot be created or written.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::WriterBuilder;
    /// let mut writer = WriterBuilder::new("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// # Ok(()) }
    /// ```
    pub fn finish(self) -> Result<Writer, Error> {
        Writer::create(self.path, self.header, &self.ext_header)
    }

    /// Build a memory-mapped writer.
    ///
    /// Equivalent to [`finish`](Self::finish) but uses memory-mapped output
    /// (requires the `mmap` feature).
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::WriterBuilder;
    /// let mut writer = WriterBuilder::new("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish_mmap()?;
    /// # Ok(()) }
    /// ```
    #[cfg(feature = "mmap")]
    pub fn finish_mmap(self) -> Result<Writer, Error> {
        Writer::create_mmap(self.path, self.header, &self.ext_header)
    }

    /// Build a gzip-compressed writer.
    ///
    /// Because gzip does not support random access, the entire file is buffered
    /// in memory and compressed only on finalize.
    /// For large volumes consider using [`finish`](Self::finish) instead.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::WriterBuilder;
    /// let mut writer = WriterBuilder::new("output.mrc.gz")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish_gzip()?;
    /// # Ok(()) }
    /// ```
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
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::WriterBuilder;
    /// let mut writer = WriterBuilder::new("output.mrc.bz2")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish_bzip2()?;
    /// # Ok(()) }
    /// ```
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

    /// Build an in-memory writer.
    ///
    /// The returned writer buffers the complete MRC file (header, extended
    /// header, and voxel data) in memory until [`finalize`](Writer::finalize).
    /// Retrieve the bytes with [`Writer::into_buffer`]. Useful for testing or
    /// generating MRC data without touching the filesystem.
    ///
    /// # Examples
    /// ```
    /// use mrc::{WriterBuilder, DataBlock, OwnedData};
    ///
    /// let mut writer = WriterBuilder::new("ignored")
    ///     .shape([4, 4, 1])
    ///     .mode::<f32>()
    ///     .finish_buffer()
    ///     .unwrap();
    /// let block = DataBlock::Owned {
    ///     offset: [0, 0, 0],
    ///     shape: [4, 4, 1],
    ///     data: OwnedData::Float32(vec![0.0f32; 16]),
    /// };
    /// writer.write_data_block(&block).unwrap();
    /// writer.finalize().unwrap();
    /// let bytes = writer.into_buffer().unwrap();
    /// assert_eq!(bytes.len(), 1024 + 64);
    /// ```
    pub fn finish_buffer(self) -> Result<Writer, Error> {
        Writer::create_buffer(self.header, &self.ext_header)
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
    finalized: bool,
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

impl Drop for Writer {
    fn drop(&mut self) {
        if !self.finalized {
            tracing::warn!("Writer dropped without calling finalize() — header on disk is stale.");
        }
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
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{Header, Writer};
    /// let header = Header::new();
    /// let mut writer = Writer::from_writer_mmap("output.mrc", header, &[])?;
    /// # Ok(()) }
    /// ```
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
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{Header, Writer, CompressionLevel};
    /// let header = Header::new();
    /// let mut writer = Writer::from_writer_gzip("output.mrc.gz", header, &[], CompressionLevel::Balanced)?;
    /// # Ok(()) }
    /// ```
    #[cfg(feature = "gzip")]
    pub fn from_writer_gzip<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
        compression: CompressionLevel,
    ) -> Result<Self, Error> {
        Self::create_compressed(path, header, ext_header, compression, true)
    }

    /// Create a bzip2-compressed writer from a [`Header`] directly.
    ///
    /// Like [`from_writer`](Self::from_writer) but buffers the entire file in
    /// memory and bzip2-compresses it on [`finalize`](Self::finalize).
    /// Requires the `bzip2` feature.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{Header, Writer, CompressionLevel};
    /// let header = Header::new();
    /// let mut writer = Writer::from_writer_bzip2("output.mrc.bz2", header, &[], CompressionLevel::Balanced)?;
    /// # Ok(()) }
    /// ```
    #[cfg(feature = "bzip2")]
    pub fn from_writer_bzip2<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
        compression: CompressionLevel,
    ) -> Result<Self, Error> {
        Self::create_compressed(path, header, ext_header, compression, false)
    }

    /// Common initialization shared by all constructors.
    /// Sets endian, validates, and extracts derived metadata.
    fn init_writer(mut header: Header) -> Result<(Header, u64, Mode, usize, VolumeShape), Error> {
        header.set_file_endian(FileEndian::LittleEndian);
        header.validate_detailed()?;
        // Reject volumes whose byte size cannot be represented; otherwise
        // offset arithmetic during writes would silently wrap.
        header.data_size().ok_or(Error::InvalidHeader)?;
        let data_offset = header.data_offset() as u64;
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let bytes_per_voxel = mode.byte_size();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);
        Ok((header, data_offset, mode, bytes_per_voxel, shape))
    }

    const MODE3_WARNING: &str =
        "Mode 3 (Int16Complex) is obsolete and should not be used for writing new files.";

    pub(crate) fn create<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        // Validate before opening/truncating the destination: an invalid
        // header must not destroy an existing file.
        let (header, ..) = Self::init_writer(header)?;
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
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        let (header, data_offset, mode, bytes_per_voxel, shape) = Self::init_writer(header)?;

        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        io.write_all(&header_bytes)?;

        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            if ext_header.len() >= ext_size {
                io.write_all(&ext_header[..ext_size])?;
            } else {
                io.write_all(ext_header)?;
                let remaining = ext_size - ext_header.len();
                let mut to_write = remaining;
                const ZBUF: [u8; 4096] = [0u8; 4096];
                while to_write > 0 {
                    let n = to_write.min(4096);
                    io.write_all(&ZBUF[..n])?;
                    to_write -= n;
                }
            }
        }

        if mode == Mode::Int16Complex {
            tracing::warn!("{}", Self::MODE3_WARNING);
        }

        Ok(Self {
            header,
            data_offset,
            bytes_per_voxel,
            mode,
            shape,
            sink: DataSink::File(io),
            finalized: false,
        })
    }

    /// Create a memory-mapped writer.
    #[cfg(feature = "mmap")]
    pub(crate) fn create_mmap<P: AsRef<std::path::Path>>(
        path: P,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        let (header, data_offset, mode, bytes_per_voxel, shape) = Self::init_writer(header)?;

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
                    let mut to_write = ext_size - ext_header.len();
                    const ZBUF: [u8; 4096] = [0u8; 4096];
                    while to_write > 0 {
                        let n = to_write.min(4096);
                        (&file).write_all(&ZBUF[..n])?;
                        to_write -= n;
                    }
                }
            }
            // SAFETY: The file has been pre-sized via `set_len` and the
            // header + extended header bytes have been written. The mapping is
            // backed by a real file on local storage. The file descriptor is
            // closed after `map_mut` returns, but the mapping itself owns the
            // underlying OS object (a file mapping survives descriptor close
            // on Unix and Windows), so all accesses through the returned
            // `MmapMut` remain valid.
            unsafe {
                memmap2::MmapOptions::new()
                    .map_mut(&file)
                    .map_err(|_| Error::Mmap)?
            }
        };

        if mode == Mode::Int16Complex {
            tracing::warn!("{}", Self::MODE3_WARNING);
        }

        Ok(Self {
            header,
            data_offset,
            bytes_per_voxel,
            mode,
            shape,
            sink: DataSink::Mmap(mmap),
            finalized: false,
        })
    }

    /// Create a fully in-memory writer, pre-sized to the declared volume.
    fn create_buffer(header: Header, ext_header: &[u8]) -> Result<Self, Error> {
        let (header, data_offset, mode, bytes_per_voxel, shape) = Self::init_writer(header)?;
        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let total = usize::try_from(data_offset)
            .map_err(|_| Error::SizeOverflow)?
            .checked_add(data_size)
            .ok_or(Error::SizeOverflow)?;
        let mut buf = vec![0u8; total];
        let mut hb = [0u8; 1024];
        header.encode_to_bytes(&mut hb);
        buf[..1024].copy_from_slice(&hb);
        let ext_size = header.nsymbt as usize;
        if ext_size > 0 {
            let n = ext_header.len().min(ext_size);
            buf[1024..1024 + n].copy_from_slice(&ext_header[..n]);
        }

        if mode == Mode::Int16Complex {
            tracing::warn!("{}", Self::MODE3_WARNING);
        }

        Ok(Self {
            header,
            data_offset,
            bytes_per_voxel,
            mode,
            shape,
            sink: DataSink::Buffer(buf),
            finalized: false,
        })
    }

    /// Create a compressed writer.
    #[cfg(any(feature = "gzip", feature = "bzip2"))]
    pub(crate) fn create_compressed<P: AsRef<std::path::Path>>(
        path: P,
        mut header: Header,
        ext_header: &[u8],
        compression: CompressionLevel,
        is_gzip: bool,
    ) -> Result<Self, Error> {
        if !ext_header.is_empty() {
            header.nsymbt = ext_header.len() as i32;
        }
        let (header, data_offset, mode, bytes_per_voxel, shape) = Self::init_writer(header)?;

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
        let total = off.checked_add(data_size).ok_or(Error::SizeOverflow)?;
        let mut buf = vec![0u8; total];
        let mut hb = [0u8; 1024];
        header.encode_to_bytes(&mut hb);
        buf[..1024].copy_from_slice(&hb);
        if ext_size > 0 {
            buf[1024..1024 + ext_size].copy_from_slice(&ext_stored);
        }

        if mode == Mode::Int16Complex {
            tracing::warn!("{}", Self::MODE3_WARNING);
        }

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
            finalized: false,
        })
    }

    /// Volume dimensions for this writer.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::create;
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// assert_eq!(writer.shape().nx, 64);
    /// # Ok(()) }
    /// ```
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode for this writer.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::create;
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// assert_eq!(writer.mode(), mrc::Mode::Float32);
    /// # Ok(()) }
    /// ```
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Read-only reference to the current header.
    ///
    /// For mutable access, use [`header_mut`](Self::header_mut).
    /// Modify header fields before calling [`finalize`](Self::finalize) to
    /// change what gets written to disk.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::create;
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// let h = writer.header();
    /// assert_eq!(h.nx, 64);
    /// # Ok(()) }
    /// ```
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Mutable reference to the current header.
    ///
    /// Allows modifying metadata fields (labels, density statistics, space
    /// group, sampling) between writing blocks and calling
    /// [`finalize`](Self::finalize).
    ///
    /// The data layout fields — `nx`, `ny`, `nz`, `mode`, and `nsymbt` — are
    /// fixed when the writer is created. Changing them causes
    /// [`finalize`](Self::finalize) and
    /// [`update_header_stats`](Self::update_header_stats) to return
    /// [`Error::WriterHeaderChanged`].
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::create;
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 64])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// writer.header_mut().ispg = 1;
    /// # Ok(()) }
    /// ```
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    /// Consume the writer and return the in-memory MRC file produced by
    /// [`WriterBuilder::finish_buffer`].
    ///
    /// Call [`finalize`](Self::finalize) first so the header contains the
    /// final metadata. Returns [`Error::NotInMemory`] for file, mmap, or
    /// compressed writers.
    ///
    /// # Examples
    /// ```no_run
    /// use mrc::create;
    ///
    /// let mut writer = create("ignored")
    ///     .shape([4, 4, 1])
    ///     .mode::<f32>()
    ///     .finish_buffer()?;
    /// // ... write blocks ...
    /// writer.finalize()?;
    /// let bytes: Vec<u8> = writer.into_buffer()?;
    /// # Ok::<_, mrc::Error>(())
    /// ```
    pub fn into_buffer(mut self) -> Result<Vec<u8>, Error> {
        match &mut self.sink {
            DataSink::Buffer(buf) => Ok(std::mem::take(buf)),
            _ => Err(Error::NotInMemory),
        }
    }

    /// Reject header edits that change the data layout after creation.
    ///
    /// `header_mut` is intended for metadata (labels, statistics, space group),
    /// not for dimensions, mode, or extended-header size: changing those would
    /// make the header describe a different layout than the bytes already
    /// written.
    fn check_layout_unchanged(&self) -> Result<(), Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let header = &self.header;
        if header.nx < 0
            || header.ny < 0
            || header.nz < 0
            || header.nx as usize != nx
            || header.ny as usize != ny
            || header.nz as usize != nz
            || header.mode != self.mode.as_i32()
            || header.data_offset() as u64 != self.data_offset
        {
            return Err(Error::WriterHeaderChanged);
        }
        Ok(())
    }

    /// Validate that `shape` fits in the volume and that `actual` equals the
    /// number of voxels it describes.
    fn validate_voxel_block(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
        actual: usize,
    ) -> Result<(), Error> {
        if !self.shape.contains_block(offset, shape) {
            return Err(Error::BoundsError {
                offset: Some(offset),
                shape: Some(shape),
                volume: Some([self.shape.nx, self.shape.ny, self.shape.nz]),
            });
        }
        let expected = shape[0]
            .checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or(Error::SizeOverflow)?;
        if actual != expected {
            return Err(Error::BlockShapeMismatch { expected, actual });
        }
        Ok(())
    }

    /// Validate a packed 4-bit block; `actual` is the number of packed bytes.
    fn validate_packed_block(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
        actual: usize,
    ) -> Result<(), Error> {
        if !self.shape.contains_block(offset, shape) {
            return Err(Error::BoundsError {
                offset: Some(offset),
                shape: Some(shape),
                volume: Some([self.shape.nx, self.shape.ny, self.shape.nz]),
            });
        }
        let expected = shape[0]
            .div_ceil(2)
            .checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or(Error::SizeOverflow)?;
        if actual != expected {
            return Err(Error::BlockShapeMismatch { expected, actual });
        }
        Ok(())
    }

    /// Write `bytes` at an absolute byte offset, dispatching once on the sink.
    fn write_at(&mut self, byte_offset: u64, bytes: &[u8]) -> Result<(), Error> {
        match &mut self.sink {
            DataSink::File(io) => {
                io.seek(SeekFrom::Start(byte_offset))?;
                io.write_all(bytes)?;
                Ok(())
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => {
                let (start, end) = checked_byte_range(byte_offset, bytes.len(), mmap.len())?;
                mmap[start..end].copy_from_slice(bytes);
                Ok(())
            }
            DataSink::Compressed { buf, .. } | DataSink::Buffer(buf) => {
                let (start, end) = checked_byte_range(byte_offset, bytes.len(), buf.len())?;
                buf[start..end].copy_from_slice(bytes);
                Ok(())
            }
        }
    }

    /// Read exactly `out.len()` bytes from an absolute byte offset.
    fn read_at(&mut self, byte_offset: u64, out: &mut [u8]) -> Result<(), Error> {
        match &mut self.sink {
            DataSink::File(io) => {
                io.seek(SeekFrom::Start(byte_offset))?;
                io.read_exact(out)?;
                Ok(())
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => {
                let (start, end) = checked_byte_range(byte_offset, out.len(), mmap.len())?;
                out.copy_from_slice(&mmap[start..end]);
                Ok(())
            }
            DataSink::Compressed { buf, .. } | DataSink::Buffer(buf) => {
                let (start, end) = checked_byte_range(byte_offset, out.len(), buf.len())?;
                out.copy_from_slice(&buf[start..end]);
                Ok(())
            }
        }
    }

    /// Write an entire volume's worth of data and compute density statistics.
    ///
    /// This is a convenience over manual [`write_data_block`](Self::write_data_block) +
    /// [`update_header_stats`](Self::update_header_stats) calls — it writes the
    /// data then immediately scans and updates header statistics.
    ///
    /// The data must cover the full volume (its length must match
    /// `nx × ny × nz`).
    ///
    /// For partial writes, use [`write_data_block`](Self::write_data_block) directly.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::create;
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 32])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// let data = vec![0.0f32; 64 * 64 * 32];
    /// writer.set_data(&data)?;
    /// writer.finalize()?;
    /// # Ok(()) }
    /// ```
    pub fn set_data<T: Voxel>(&mut self, data: &[T]) -> Result<(), Error> {
        if T::MODE != self.mode {
            return Err(Error::ModeMismatch {
                file_mode: self.mode,
                requested_mode: T::MODE,
                offset: Some([0, 0, 0]),
            });
        }
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        let expected = nx
            .checked_mul(ny)
            .and_then(|v| v.checked_mul(nz))
            .ok_or(Error::SizeOverflow)?;
        if data.len() != expected {
            return Err(Error::TypeMismatch {
                expected,
                actual: data.len(),
            });
        }
        self.write_block_data::<T>([0, 0, 0], [nx, ny, nz], data)?;
        self.update_header_stats()?;
        Ok(())
    }

    /// Write a block of voxels to the file using runtime mode dispatch.
    ///
    /// Accepts a [`DataBlock`](crate::DataBlock) of any mode matching the file's
    /// mode. For full XY slabs with the `parallel` feature, encoding is
    /// performed in parallel automatically.
    ///
    /// Supports arbitrary sub-blocks by scattering row-by-row when necessary.
    ///
    /// # Errors
    /// Returns [`Error::ModeMismatch`] if the block's payload type does not
    /// match the writer's mode, [`Error::BlockShapeMismatch`] if the payload
    /// length does not match the block shape, and [`Error::BoundsError`] if the
    /// block lies outside the volume. The checks run before any I/O.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{create, DataBlock, OwnedData};
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 1])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// let block = DataBlock::Owned {
    ///     offset: [0, 0, 0],
    ///     shape: [64, 64, 1],
    ///     data: OwnedData::Float32(vec![0.0f32; 64 * 64]),
    /// };
    /// writer.write_data_block(&block)?;
    /// # Ok(()) }
    /// ```
    pub fn write_data_block(&mut self, block: &DataBlock<'_>) -> Result<(), Error> {
        let offset = block.offset();
        let shape = block.shape();
        let view = block.data();

        // The block payload type must match the file's declared mode. Without
        // this check a mismatched block silently writes the wrong number of
        // bytes per voxel and corrupts the output.
        if view.mode() != self.mode {
            return Err(Error::ModeMismatch {
                file_mode: self.mode,
                requested_mode: view.mode(),
                offset: Some(offset),
            });
        }

        match view {
            DataView::Int8(data) => self.write_block_data::<i8>(offset, shape, data),
            DataView::Int16(data) => self.write_block_data::<i16>(offset, shape, data),
            DataView::Float32(data) => self.write_block_data::<f32>(offset, shape, data),
            DataView::Uint16(data) => self.write_block_data::<u16>(offset, shape, data),
            DataView::Int16Complex(data) => {
                self.write_block_data::<Int16Complex>(offset, shape, data)
            }
            DataView::Float32Complex(data) => {
                self.write_block_data::<Float32Complex>(offset, shape, data)
            }
            #[cfg(feature = "f16")]
            DataView::Float16(data) => self.write_block_data::<crate::f16>(offset, shape, data),
            DataView::Packed4Bit(data) => self.write_block_bytes(data, offset, shape),
        }
    }

    /// Core write implementation: encode and persist typed voxel data.
    ///
    /// Validates the mode, bounds, and data length before any I/O.
    fn write_block_data<T: Voxel>(
        &mut self,
        offset: [usize; 3],
        shape: [usize; 3],
        data: &[T],
    ) -> Result<(), Error> {
        if T::MODE != self.mode {
            return Err(Error::ModeMismatch {
                file_mode: self.mode,
                requested_mode: T::MODE,
                offset: Some(offset),
            });
        }
        self.validate_voxel_block(offset, shape, data.len())?;

        let file_endian = self.header.detect_endian();
        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;
        let b = self.bytes_per_voxel;

        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear =
                (ox as u64) + (oy as u64) * (nx as u64) + (oz as u64) * (nx as u64) * (ny as u64);
            let start_offset = self.data_offset + linear * (b as u64);

            if file_endian == FileEndian::native() {
                // SAFETY: `T::MODE == self.mode` and every `Voxel` type is a
                // plain-old-data numeric type. `data` contains exactly the
                // number of elements the block shape declares (checked above),
                // so the byte view covers the whole block.
                let bytes = unsafe {
                    core::slice::from_raw_parts(
                        data.as_ptr().cast::<u8>(),
                        core::mem::size_of_val(data),
                    )
                };
                return self.write_at(start_offset, bytes);
            }

            // Non-native endian: encode the whole slab before writing.
            #[cfg(feature = "parallel")]
            {
                let encoded = encode_block_parallel(data, file_endian)?;
                return self.write_at(start_offset, &encoded);
            }
            #[cfg(not(feature = "parallel"))]
            {
                let plane_voxels = sx * sy;
                let plane_bytes = plane_voxels * b;
                let mut plane_buf = vec![0u8; plane_bytes];
                for z in 0..sz {
                    let plane = &data[z * plane_voxels..(z + 1) * plane_voxels];
                    encode_slice(plane, &mut plane_buf, file_endian)?;
                    self.write_at(start_offset + (z as u64) * (plane_bytes as u64), &plane_buf)?;
                }
                return Ok(());
            }
        }

        // Full-width rows are contiguous, so a partial-Y block can be written
        // as one run per Z-plane instead of one write per row.
        if ox == 0 && sx == nx {
            let plane_values = sx * sy;
            let plane_bytes = plane_values * b;
            let mut encoded = if file_endian == FileEndian::native() {
                None
            } else {
                Some(vec![0u8; plane_bytes])
            };
            for z in 0..sz {
                let start_offset =
                    self.data_offset + (((oz + z) * ny + oy) * nx) as u64 * (b as u64);
                let values = &data[z * plane_values..(z + 1) * plane_values];
                match &mut encoded {
                    None => {
                        // SAFETY: `T` is a POD voxel type and the slice is
                        // exactly `plane_values` elements (validated above).
                        let bytes = unsafe {
                            core::slice::from_raw_parts(
                                values.as_ptr().cast::<u8>(),
                                core::mem::size_of_val(values),
                            )
                        };
                        self.write_at(start_offset, bytes)?;
                    }
                    Some(buf) => {
                        encode_slice(values, buf, file_endian)?;
                        self.write_at(start_offset, buf)?;
                    }
                }
            }
            return Ok(());
        }

        let mut row_bytes = vec![0u8; sx * b];
        for z in 0..sz {
            for y in 0..sy {
                let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
                let file_offset = self.data_offset + (file_linear as u64) * (b as u64);
                let block_idx = y * sx + z * sx * sy;
                let row_values = &data[block_idx..block_idx + sx];
                encode_slice(row_values, &mut row_bytes, file_endian)?;
                self.write_at(file_offset, &row_bytes)?;
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
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{create, VoxelBlock};
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 1])
    ///     .mode::<u16>()
    ///     .finish()?;
    /// let block = VoxelBlock::new([0, 0, 0], [64, 64, 1], vec![0u8; 64 * 64])?;
    /// writer.write_u8_block(&block)?;
    /// # Ok(()) }
    /// ```
    pub fn write_u8_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
                offset: Some(block.offset),
            });
        }
        self.validate_voxel_block(block.offset, block.shape, block.data.len())?;
        let file_endian = self.header.detect_endian();
        let bytes = crate::engine::convert::convert_u8_to_u16_bytes(&block.data, file_endian);
        self.write_encoded_data(&bytes, block.offset, block.shape)
    }

    /// Write a block with automatic type conversion to the file's mode.
    ///
    /// The input data is converted to `f32` first, then encoded to the file's
    /// on-disk mode. Supported input types: `i8`, `i16`, `u16`, `f32`, `f16`.
    /// Supported target modes:
    ///
    /// | File mode | Conversion |
    /// |-----------|------------|
    /// | [`Int8`](Mode::Int8) | `f32` → `i8` (clamped, rounded to nearest even) |
    /// | [`Int16`](Mode::Int16) | `f32` → `i16` (clamped, rounded to nearest even) |
    /// | [`Uint16`](Mode::Uint16) | `f32` → `u16` (clamped, rounded to nearest even) |
    /// | [`Float32`](Mode::Float32) | `f32` → `f32` (identity encode) |
    /// | [`Float16`](Mode::Float16) | `f32` → `f16` (SIMD, requires `f16` feature) |
    ///
    /// Complex modes and `Packed4Bit` are not convertible from real f32 data.
    /// Use [`write_data_block`](Writer::write_data_block) with a matching complex data block instead.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{create, VoxelBlock};
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 1])
    ///     .mode::<u16>()
    ///     .finish()?;
    /// // write_block_as accepts f32, i16, i8, u16, or f16 — any type that
    /// // can be converted to f32. Conversion is automatic.
    /// let block = VoxelBlock::new([0, 0, 0], [64, 64, 1], vec![0.0f32; 64 * 64])?;
    /// writer.write_block_as(&block)?;
    /// # Ok(()) }
    /// ```
    pub fn write_block_as<T>(&mut self, block: &VoxelBlock<T>) -> Result<(), Error>
    where
        T: Voxel,
        f32: crate::engine::convert::ConvertFrom<T>,
    {
        self.validate_voxel_block(block.offset, block.shape, block.data.len())?;

        // Direct path: input type matches the file mode — skip f32 conversion.
        if T::MODE == self.mode() {
            return self.write_block_data::<T>(block.offset, block.shape, &block.data);
        }

        // Fast path: T = f32 — zero-copy, avoids the identity ConvertFrom copy.
        if TypeId::of::<T>() == TypeId::of::<f32>() {
            // SAFETY: TypeId confirms T == f32. VoxelBlock<T> has identical
            // field layout for all T (offset, shape, data). The macro only
            // borrows, never moves, so reinterpreting the reference is sound.
            let f32_block: &VoxelBlock<f32> =
                unsafe { &*(block as *const VoxelBlock<T> as *const VoxelBlock<f32>) };
            return write_block_as_body!(self, f32_block);
        }

        let f32_data = <f32 as crate::engine::convert::ConvertFrom<T>>::convert_from(&block.data);
        let f32_block = VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data: f32_data,
        };
        write_block_as_body!(self, f32_block)
    }

    /// Write pre-encoded bytes at the given offset and shape.
    ///
    /// Handles row-by-row scatter for sub-XY blocks. Used by fused
    /// writer paths that produce `Vec<u8>` directly.
    fn write_encoded_data(
        &mut self,
        bytes: &[u8],
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<(), Error> {
        let b = self.bytes_per_voxel;
        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if !self.shape.contains_block(offset, shape) {
            return Err(Error::BoundsError {
                offset: Some(offset),
                shape: Some(shape),
                volume: Some([nx, ny, self.shape.nz]),
            });
        }
        let expected = sx
            .checked_mul(sy)
            .and_then(|v| v.checked_mul(sz))
            .and_then(|v| v.checked_mul(b))
            .ok_or(Error::SizeOverflow)?;
        if bytes.len() != expected {
            return Err(Error::TypeMismatch {
                expected,
                actual: bytes.len(),
            });
        }

        let block_row_bytes = sx * b;

        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let linear =
                (ox as u64) + (oy as u64) * (nx as u64) + (oz as u64) * (nx as u64) * (ny as u64);
            let start_offset = self.data_offset + linear * (b as u64);
            return self.write_at(start_offset, bytes);
        }

        // Full-width rows are contiguous: write one run per Z-plane.
        if ox == 0 && sx == nx {
            let plane_bytes = sx * sy * b;
            for z in 0..sz {
                let dst = self.data_offset + (((oz + z) * ny + oy) * nx) as u64 * (b as u64);
                let src = z * plane_bytes;
                self.write_at(dst, &bytes[src..src + plane_bytes])?;
            }
            return Ok(());
        }

        for z in 0..sz {
            for y in 0..sy {
                let lin = ox + (oy + y) * nx + (oz + z) * nx * ny;
                let dst = self.data_offset + (lin as u64) * (b as u64);
                let src = (y * sx + z * sx * sy) * b;
                self.write_at(dst, &bytes[src..src + block_row_bytes])?;
            }
        }
        Ok(())
    }

    /// Write a block of `u8` data (0–15 per voxel) by packing to 4-bit (Mode 101).
    ///
    /// The file must have been created with [`Mode::Packed4Bit`]. Each `u8` value
    /// is checked to be in the range 0–15; values exceeding 15 produce an error.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{create, VoxelBlock};
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 1])
    ///     .mode_raw(101)  // Packed4Bit
    ///     .finish()?;
    /// let data = vec![0u8; 64 * 64];
    /// let block = VoxelBlock::new([0, 0, 0], [64, 64, 1], data)?;
    /// writer.write_u4_block(&block)?;
    /// # Ok(()) }
    /// ```
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
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if ox != 0 {
            return Err(Error::BoundsError {
                offset: Some(offset),
                shape: Some(shape),
                volume: Some([nx, ny, nz]),
            });
        }
        self.validate_packed_block(offset, shape, packed.len())?;

        let file_row_bytes = nx.div_ceil(2);
        let block_row_bytes = sx.div_ceil(2);

        if sx == nx && oy == 0 && sy == ny {
            let slice_bytes = ny * file_row_bytes;
            let start_offset = self.data_offset + (oz as u64) * (slice_bytes as u64);
            let byte_len = sz * slice_bytes;
            return self.write_at(start_offset, &packed[..byte_len]);
        }

        // Full-width rows are contiguous: write one run per Z-plane.
        if sx == nx {
            let plane_bytes = sy * file_row_bytes;
            for z in 0..sz {
                let vol_row = (oz + z) * ny + oy;
                let file_offset = self.data_offset + (vol_row as u64) * (file_row_bytes as u64);
                let src = z * plane_bytes;
                self.write_at(file_offset, &packed[src..src + plane_bytes])?;
            }
            return Ok(());
        }

        for z in 0..sz {
            for y in 0..sy {
                let vol_row = (oz + z) * ny + (oy + y);
                let file_offset = self.data_offset + (vol_row as u64) * (file_row_bytes as u64);
                let packed_start = (y + z * sy) * block_row_bytes;
                let packed_end = packed_start + block_row_bytes;
                self.write_at(file_offset, &packed[packed_start..packed_end])?;
                // Sub-width odd block: the high nibble of the last byte
                // belongs to a pixel inside the volume but outside this block.
                if sx % 2 != 0 {
                    let pos = file_offset + (block_row_bytes as u64) - 1;
                    let mut last = [0u8];
                    self.read_at(pos, &mut last)?;
                    last[0] = (last[0] & 0xF0) | (packed[packed_end - 1] & 0x0F);
                    self.write_at(pos, &last)?;
                }
            }
        }
        Ok(())
    }

    /// Finalize the MRC file by rewriting the header.
    ///
    /// For plain-file writers this verifies that the file contains exactly the
    /// declared header + voxel bytes; an incomplete file is reported as
    /// [`Error::FileSizeMismatch`]. Header metadata edited through
    /// [`header_mut`](Self::header_mut) is validated before it is written.
    ///
    /// # Errors
    /// Returns [`Error::WriterHeaderChanged`] if the layout fields changed,
    /// [`Error::InvalidHeaderDetailed`] if the edited header is invalid, and
    /// [`Error::FileSizeMismatch`] for an incomplete plain file.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{create, DataBlock, OwnedData};
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 1])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// let block = DataBlock::Owned {
    ///     offset: [0, 0, 0],
    ///     shape: [64, 64, 1],
    ///     data: OwnedData::Float32(vec![0.0f32; 64 * 64]),
    /// };
    /// writer.write_data_block(&block)?;
    /// writer.finalize()?;
    /// # Ok(()) }
    /// ```
    pub fn finalize(&mut self) -> Result<(), Error> {
        self.check_layout_unchanged()?;
        // `header_mut` may have edited metadata; never write a header that
        // fails the crate's own validation.
        self.header.validate_detailed()?;
        let data_size = u64::try_from(self.header.data_size().ok_or(Error::InvalidHeader)?)
            .map_err(|_| Error::SizeOverflow)?;
        let expected_total = self
            .data_offset
            .checked_add(data_size)
            .ok_or(Error::SizeOverflow)?;
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);

        let result = match &mut self.sink {
            DataSink::File(io) => {
                // A plain-file writer grows the file as blocks are written.
                // Refuse to finalize an incomplete (or oversized) file: the
                // result could not be reopened in strict mode.
                let actual = io.seek(SeekFrom::End(0))?;
                if actual != expected_total {
                    return Err(Error::FileSizeMismatch {
                        expected: usize::try_from(expected_total).unwrap_or(usize::MAX),
                        actual: usize::try_from(actual).unwrap_or(usize::MAX),
                    });
                }
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
            #[allow(unused_variables)]
            DataSink::Compressed {
                buf,
                path,
                compression,
                is_gzip,
            } => {
                buf[..1024].copy_from_slice(&header_bytes);
                #[cfg(any(feature = "gzip", feature = "bzip2"))]
                {
                    let compressed = compress_data(buf, *compression, *is_gzip)?;
                    std::fs::write(path, compressed)?;
                    Ok(())
                }
                #[cfg(not(any(feature = "gzip", feature = "bzip2")))]
                Err(Error::UnsupportedMode)
            }
            DataSink::Buffer(buf) => {
                if buf.len() < 1024 {
                    return Err(Error::bounds_err());
                }
                buf[..1024].copy_from_slice(&header_bytes);
                Ok(())
            }
        };
        if result.is_ok() {
            self.finalized = true;
        }
        result
    }

    /// Scan the written volume and update header statistics.
    ///
    /// Honors IMOD's unsigned Mode 0 convention when computing statistics.
    ///
    /// # Errors
    /// Returns [`Error::WriterHeaderChanged`] if the data layout fields were
    /// modified through [`header_mut`](Self::header_mut), or an I/O error if
    /// the written data cannot be read back.
    ///
    /// # Examples
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use mrc::{create, DataBlock, OwnedData};
    /// let mut writer = create("output.mrc")
    ///     .shape([64, 64, 1])
    ///     .mode::<f32>()
    ///     .finish()?;
    /// let block = DataBlock::Owned {
    ///     offset: [0, 0, 0],
    ///     shape: [64, 64, 1],
    ///     data: OwnedData::Float32(vec![0.0f32; 64 * 64]),
    /// };
    /// writer.write_data_block(&block)?;
    /// writer.update_header_stats()?;
    /// writer.finalize()?;
    /// # Ok(()) }
    /// ```
    pub fn update_header_stats(&mut self) -> Result<(), Error> {
        self.check_layout_unchanged()?;
        let (data_offset, data_size) = {
            let ds = self.header.data_size().ok_or(Error::InvalidHeader)?;
            (self.header.data_offset(), ds)
        };
        match &mut self.sink {
            DataSink::File(io) => {
                #[allow(unused_mut)]
                let (dmin, dmax, dmean, rms) = {
                    use std::io::Read;
                    let mode = Mode::from_i32(self.header.mode).ok_or(Error::UnsupportedMode)?;
                    let endian = self.header.detect_endian();
                    let nx = self.header.nx.max(0) as usize;
                    let b = self.bytes_per_voxel;
                    let m0_unsigned = mode == Mode::Int8
                        && self
                            .header
                            .detect_imod()
                            .is_some_and(|imod| !imod.bytes_are_signed);
                    io.seek(SeekFrom::Start(self.data_offset))?;

                    // Chunked path: avoids full-volume buffer for real-valued modes.
                    // Complex modes fall back to the full read.
                    match mode {
                        Mode::Float32
                        | Mode::Int16
                        | Mode::Uint16
                        | Mode::Int8
                        | Mode::Packed4Bit
                        | Mode::Float16 => {
                            let chunk_bytes = if mode == Mode::Packed4Bit {
                                let row_bytes = nx.div_ceil(2);
                                (262_144 / row_bytes).max(1) * row_bytes
                            } else {
                                262_144 * b
                            };
                            let mut chunk = vec![0u8; chunk_bytes];
                            let mut stats = crate::engine::stats::RunningStats::new();
                            let mut remaining = data_size;

                            while remaining > 0 {
                                let to_read = chunk_bytes.min(remaining);
                                let buf = &mut chunk[..to_read];
                                io.read_exact(buf)?;
                                match mode {
                                    Mode::Float32 => {
                                        let d =
                                            crate::engine::codec::decode_slice::<f32>(buf, endian)?;
                                        stats.update(&d);
                                    }
                                    Mode::Int16 => {
                                        let d =
                                            crate::engine::codec::decode_slice::<i16>(buf, endian)?;
                                        stats.update(&d);
                                    }
                                    Mode::Uint16 => {
                                        let d =
                                            crate::engine::codec::decode_slice::<u16>(buf, endian)?;
                                        stats.update(&d);
                                    }
                                    Mode::Int8 => {
                                        if m0_unsigned {
                                            let d = crate::engine::convert::reinterpret_m0(
                                                buf,
                                                crate::M0Interpretation::Unsigned,
                                            );
                                            stats.update(&d);
                                        } else {
                                            let d = crate::engine::codec::decode_slice::<i8>(
                                                buf, endian,
                                            )?;
                                            stats.update(&d);
                                        }
                                    }
                                    #[cfg(feature = "f16")]
                                    Mode::Float16 => {
                                        let n = to_read / 2;
                                        for i in 0..n {
                                            let v = crate::f16::from_bytes(buf, i * 2, endian);
                                            stats.push(f32::from(v) as f64);
                                        }
                                    }
                                    #[cfg(not(feature = "f16"))]
                                    Mode::Float16 => return Err(Error::UnsupportedMode),
                                    Mode::Packed4Bit => {
                                        let row_bytes = nx.div_ceil(2);
                                        let rows_in_chunk = to_read / row_bytes;
                                        let unpacked =
                                            crate::engine::convert::unpack_u4_bytes_to_u8(
                                                buf,
                                                nx,
                                                rows_in_chunk,
                                            );
                                        stats.update(&unpacked);
                                    }
                                    _ => unreachable!(),
                                }
                                remaining -= to_read;
                            }
                            stats.finalize()
                        }
                        _ => {
                            // Fallback for complex and other modes: read full data.
                            let mut buf = vec![0u8; data_size];
                            io.read_exact(&mut buf)?;
                            crate::engine::stats::compute_stats(
                                &buf,
                                mode,
                                endian,
                                self.header.nx.max(0) as usize,
                                self.header.ny.max(0) as usize * self.header.nz.max(0) as usize,
                            )?
                        }
                    }
                };
                self.header.dmin = dmin;
                self.header.dmax = dmax;
                self.header.dmean = dmean;
                self.header.rms = rms;
                Ok(())
            }
            #[cfg(feature = "mmap")]
            DataSink::Mmap(mmap) => {
                let start = usize::try_from(self.data_offset).map_err(|_| Error::SizeOverflow)?;
                let end = start.checked_add(data_size).ok_or(Error::SizeOverflow)?;
                if end > mmap.len() {
                    return Err(Error::bounds_err());
                }
                update_header_stats_from_bytes(&mut self.header, &mmap[start..end])
            }
            DataSink::Compressed { buf, .. } | DataSink::Buffer(buf) => {
                let end = data_offset
                    .checked_add(data_size)
                    .ok_or(Error::SizeOverflow)?;
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
fn compress_data(
    data: &[u8],
    compression: CompressionLevel,
    is_gzip: bool,
) -> Result<Vec<u8>, Error> {
    #[cfg(feature = "gzip")]
    if is_gzip {
        let mut encoder = flate2::write::GzEncoder::new(
            Vec::with_capacity(data.len() / 2),
            compression.to_flate2(),
        );
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
    let (dmin, dmax, dmean, rms) = crate::engine::stats::compute_stats_for_header(header, bytes)?;
    header.dmin = dmin;
    header.dmax = dmax;
    header.dmean = dmean;
    header.rms = rms;
    Ok(())
}
