//! Read and write MRC-2014 files — the standard in cryo-EM and structural
//! biology. Handles byte-order, type conversion, and compression so you
//! can focus on your science. SIMD-accelerated, mmap-enabled.
//!
//! # Basic usage
//!
//! These APIs are all you need for everyday work.  They return typed data
//! (zero-copy borrowed views) and auto-detect compression, endianness,
//! and the file's voxel mode.
//!
//! ## Quick start
//!
//! ```no_run
//! use mrc::{read_as, write_as};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Read an entire volume in one call
//! let (header, data): (_, Vec<f32>) = read_as("density.mrc")?;
//! println!("{}×{}×{} volume, {} voxels",
//!     header.nx, header.ny, header.nz, data.len());
//!
//! // Write a volume in one call
//! write_as("output.mrc", &data, [512, 512, 256])?;
//! # Ok(()) }
//! ```
//!
//! ## Reading data
//!
//! Open any MRC file — compression, byte order, and voxel type are
//! detected automatically.  No need to specify gzip, endianness, or mode.
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::{open, Reader};
//!
//! let reader = open("tilt_series.mrc")?;
//! println!("{}×{}×{} voxels, mode {:?}",
//!     reader.shape().nx, reader.shape().ny, reader.shape().nz,
//!     reader.mode());
//! # Ok(()) }
//! ```
//!
//! Then choose an iteration method.  Each returns [`DataBlock`] chunks
//! whose [`DataView`] variant is determined by the file's mode at runtime
//! — no compile-time guessing, no `ModeMismatch` errors:
//!
//! | Method | Returns | Use case |
//! |---|---|---|
//! | [`slices`](Reader::slices) | one Z-plane at a time | 2D processing per slice |
//! | [`slabs`](Reader::slabs) | `k` Z-planes | Batch processing |
//! | [`tiles`](Reader::tiles) | arbitrary 3D blocks | Local analysis |
//! | [`subregion`](Reader::subregion) | single block by coordinate | Random access |
//! | [`read_volume`](Reader::read_volume) | entire volume | Full-volume processing |
//! | [`volumes`](Reader::volumes) | sub-volumes | Volume stacks (ISPG 401+) |
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! for slice in reader.slices() {
//!     let block = slice?;
//!     match block.data() {
//!         mrc::DataView::Float32(data) => { /* process f32 slice */ }
//!         mrc::DataView::Int16(data)   => { /* process i16 slice */ }
//!         _ => {} /* other modes handled transparently */
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! Or use [`convert::<f32>()`](Reader::convert) — the **fire-and-forget**
//! option that reads any mode as `f32`:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! for slice in reader.convert::<f32>().slices() {
//!     let block = slice?;
//!     println!("z={}: {} voxels", block.offset[2], block.data.len());
//! }
//! // Full volume in one call:
//! let block = reader.convert::<f32>().read_volume()?;
//! # Ok(()) }
//! ```
//!
//! The same iterators — [`slabs`](Reader::slabs),
//! [`tiles`](Reader::tiles), [`subregion`](Reader::subregion) — work with
//! `convert::<f32>()` too, as do
//! [`with_complex_strategy`](crate::ConvertReader::with_complex_strategy)
//! and [`with_m0_interpretation`](crate::ConvertReader::with_m0_interpretation).
//!
//! ## Writing data
//!
//! Use [`create()`] to get a [`WriterBuilder`], set shape and mode,
//! then call [`finish`](WriterBuilder::finish).  Write data block by block
//! — each [`write_data_block`](Writer::write_data_block) encodes and writes
//! immediately (streaming, no intermediate buffering):
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::{create, DataBlock, OwnedData};
//!
//! let mut writer = create("output.mrc")
//!     .shape([256, 256, 128])
//!     .mode::<f32>()
//!     .finish()?;
//!
//! let block = DataBlock::Owned {
//!     offset: [0, 0, 0],
//!     shape: [256, 256, 1],
//!     data: OwnedData::Float32(vec![0.0f32; 256 * 256]),
//! };
//! writer.write_data_block(&block)?;
//! writer.update_header_stats()?;
//! writer.finalize()?;
//! # Ok(()) }
//! ```
//!
//! For a single call, use [`write_as()`] or [`Writer::set_data`]:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::write_as;
//! let data = vec![0.0f32; 256 * 256 * 128];
//! write_as("output.mrc", &data, [256, 256, 128])?;
//! # Ok(()) }
//! ```
//!
//! The writer lifecycle:
//! 1. Write blocks with [`write_data_block`](Writer::write_data_block)
//!    or [`write_block_as`](Writer::write_block_as) for auto-conversion.
//! 2. Call [`update_header_stats`](Writer::update_header_stats) to fill
//!    in `dmin`/`dmax`/`dmean`/`rms`.
//! 3. **Require** [`finalize`](Writer::finalize) — rewrites the header
//!    with final metadata.
//!
//! ## Data modes
//!
//! MRC files encode voxels in one of several numeric modes.
//! [`Mode`] represents them at runtime; [`Voxel`] ties each Rust type
//! to its mode at compile time.
//!
//! | Mode | Rust type | Typical use |
//! |---|---|---|
//! | [`Int8`](Mode::Int8) (0) | `i8` | Binary masks |
//! | [`Int16`](Mode::Int16) (1) | `i16` | Raw cryo-EM density |
//! | [`Float32`](Mode::Float32) (2) | `f32` | Processed / reconstructed density |
//! | [`Int16Complex`](Mode::Int16Complex) (3) | [`Int16Complex`] | Complex data (i16 real + i16 imag) |
//! | [`Float32Complex`](Mode::Float32Complex) (4) | [`Float32Complex`] | Complex data (f32 real + f32 imag) |
//! | [`Uint16`](Mode::Uint16) (6) | `u16` | Segmentation labels |
//! | [`Float16`](Mode::Float16) (12) | `f16` | Half-precision storage (feature `f16`) |
//! | [`Packed4Bit`](Mode::Packed4Bit) (101) | `u8` via [`slices_u8`](Reader::slices_u8) | 4-bit packed data |
//!
//! When you don't know the mode ahead of time, use
//! [`convert::<f32>()`](Reader::convert) which converts any mode to `f32`.
//!
//! ## Compression auto-detection
//!
//! [`Reader::open`] reads the first two bytes to detect compression:
//!
//! | Magic bytes | Format |
//! |---|---|
//! | `\x1f\x8b` | Gzip |
//! | `BZ` | Bzip2 |
//! | anything else | Plain |
//!
//! Plain files use mmap (zero-copy) or buffered I/O.  Compressed files
//! are fully decompressed into memory on open (capped at 256 GiB).
//! Use [`Reader::open_gzip_with_limit`] or
//! [`Reader::open_bzip2_with_limit`] for a custom limit.
//!
//! ## Large files
//!
//! [`Reader::open`] automatically uses memory-mapped I/O (requires the
//! `mmap` feature).  Same iterator API, zero-copy [`DataBlock`] views,
//! OS-managed paging.  For very large compressed files, decompress with
//! `gunzip`/`bunzip2` first, then use plain `Reader::open` for mmap.
//!
//! ## Reading from memory / streams
//!
//! When data is already in memory (e.g. from a camera readout or network
//! stream), use [`Reader::from_reader`] or [`Reader::from_bytes`]:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::Reader;
//! use std::io::Cursor;
//!
//! let bytes = std::fs::read("density.mrc")?;
//! let reader = Reader::from_reader(Cursor::new(bytes))?;
//! # Ok(()) }
//! ```
//!
//! ## Troubleshooting
//!
//! | Error | Likely cause | What to try |
//! |---|---|---|
//! | [`InvalidHeader`](Error::InvalidHeader) | Not an MRC file | `mrc validate file.mrc` or try [`Reader::open_permissive`] |
//! | [`FileSizeMismatch`](Error::FileSizeMismatch) | Truncated / trailing garbage | Re-download or run `mrc validate` |
//! | [`ModeMismatch`](Error::ModeMismatch) | Block type != file mode | Use [`Writer::write_block_as`] for auto-conversion |
//! | [`BoundsError`](Error::BoundsError) | Block outside volume | Check offset + shape ≤ dimensions |
//! | [`UnsupportedMode`](Error::UnsupportedMode) | Mode needs `f16` feature | Enable `f16` or convert with another tool |
//!
//! ---
//!
//! # Advanced API
//!
//! These APIs give you lower-level access to raw bytes, headers, extended
//! metadata, and validation.  They are intended for tools, pipelines, and
//! developers who need more control than the basic iterators provide.
//!
//! ## Raw byte access
//!
//! ⚠️ **The returned bytes are raw on-disk bytes** — file byte order,
//! no endian correction, no type conversion.  Use [`reader.endian()`](Reader::endian)
//! and [`reader.mode()`](Reader::mode) to interpret them correctly.
//!
//! For typed zero-copy access, use [`subregion`](Reader::subregion) /
//! [`slices`](Reader::slices) for the basic API, or
//! [`convert::<f32>()`](Reader::convert) for fire-and-forget conversion.
//!
//! | Method | Returns | Copy cost |
//! |---|---|---|
//! | [`raw_bytes`](Reader::raw_bytes) | `&[u8]` — whole data region | zero-copy |
//! | [`read_block_bytes_cow`](Reader::read_block_bytes_cow) | [`Cow<[u8]>`] — sub-block | zero-copy for contiguous XY slabs |
//! | [`read_block_bytes`](Reader::read_block_bytes) | `Vec<u8>` — sub-block | **always copies** |
//!
//! **Contiguous** = offset `[0, y, z]`, shape `[nx, ny, sz]`.  Any
//! sub-XY offset or shape forces a row-by-row gather into owned memory.
//!
//! ⚠️ `read_block_bytes` always returns an owned `Vec`, even for
//! contiguous blocks.  For large volumes this causes a full copy of the
//! block.  Use `read_block_bytes_cow` when you want to avoid the
//! allocation, or use [`subregion`](Reader::subregion) /
//! [`slices`](Reader::slices) for typed zero-copy access.
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! use std::borrow::Cow;
//!
//! // Zero-copy: borrows from mmap for contiguous blocks
//! let cow: Cow<[u8]> = reader.read_block_bytes_cow([0, 0, 0], [256, 256, 64])?;
//!
//! // Always owned: allocates + copies even for contiguous blocks
//! let bytes: Vec<u8> = reader.read_block_bytes([0, 0, 0], [256, 256, 64])?;
//! # Ok(()) }
//! ```
//!
//! ## Permissive mode / quirky files
//!
//! Common microscope quirks (NVERSION left at 0, `"MAP\0"` instead of
//! `"MAP "`) are handled transparently by [`open()`].
//!
//! For severely non-standard files, use
//! [`Reader::open_permissive`] which turns non-critical header issues
//! into warnings instead of hard errors, and allows opening files that
//! are shorter than the header declares.  Use [`is_truncated`](Reader::is_truncated)
//! to detect truncated data:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! let (reader, warnings) = mrc::Reader::open_permissive("legacy.mrc")?;
//! for w in &warnings { eprintln!("note: {w}"); }
//! if reader.is_truncated() {
//!     eprintln!("warning: file is incomplete");
//! }
//! # Ok(()) }
//! ```
//!
//! Permissive variants [`Reader::from_reader_permissive`] and
//! [`Reader::from_bytes_permissive`] are also available for in-memory
//! data.
//!
//! ## Special-mode reads (Packed4Bit and Mode 0)
//!
//! These modes have no [`Voxel`] implementation — there is no single
//! Rust type that maps to them safely at compile time — so dedicated
//! methods read directly as `u8` or `f32`:
//!
//! * [`slices_u8`](Reader::slices_u8) / [`slabs_u8`](Reader::slabs_u8) —
//!   unpack `Packed4Bit` nibbles, or narrow Uint16, to `u8`
//! * [`read_volume_u8`](Reader::read_volume_u8) — full `Packed4Bit`
//!   volume as `u8`
//! * [`slices_mode0`](Reader::slices_mode0) / [`slabs_mode0`](Reader::slabs_mode0) —
//!   read Mode 0 as `f32`, signed or unsigned
//!
//! ## Headers
//!
//! The [`Header`] struct mirrors the 1024-byte MRC-2014 fixed header.
//! Every field is a typed public member.
//!
//! ```
//! use mrc::Header;
//! let h = Header::new();
//! assert_eq!(h.map, *b"MAP ");
//! ```
//!
//! For fluent construction, use [`HeaderBuilder`]:
//!
//! ```
//! use mrc::HeaderBuilder;
//! let header = HeaderBuilder::new()
//!     .shape([512, 512, 256])
//!     .mode::<f32>()
//!     .cell_lengths(1.0, 1.0, 1.0)
//!     .build()?;
//! # Ok::<_, mrc::HeaderValidationError>(())
//! ```
//!
//! Three validation levels:
//! * [`validate`](Header::validate) — quick yes / no
//! * [`validate_detailed`](Header::validate_detailed) — specific diagnosis
//! * [`validate_permissive`](Header::validate_permissive) — warnings
//!
//! Volume type helpers (configure `ispg` and `mz`):
//!
//! ```rust
//! # use mrc::Header;
//! let mut h = Header::new();
//! h.nx = 64; h.ny = 64; h.nz = 120;
//! h.mx = 64; h.my = 64; h.mz = 30;
//! h.set_volume_stack(30);
//! assert!(h.is_volume_stack());
//! assert_eq!(h.logical_shape(), [4, 30, 64, 64]);
//! ```
//!
//! Key computed properties:
//! [`voxel_size()`](Header::voxel_size), [`sampling()`](Header::sampling),
//! [`cell_lengths()`](Header::cell_lengths), [`cell_angles()`](Header::cell_angles),
//! [`cell_volume()`](Header::cell_volume), [`density_stats()`](Header::density_stats),
//! [`logical_shape()`](Header::logical_shape), [`exttyp()`](Header::exttyp),
//! [`get_labels()`](Header::get_labels), [`detect_endian()`](Header::detect_endian),
//! [`is_standard_map()`](Header::is_standard_map), [`decode_from_bytes()`](Header::decode_from_bytes),
//! [`encode_to_bytes()`](Header::encode_to_bytes).
//!
//! ## Extended headers
//!
//! Many MRC files carry metadata after the fixed header identified by
//! a 4-byte `exttyp` field.  Use [`parse_extended_header`](Reader::parse_extended_header)
//! for auto-detection:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("file.mrc")?;
//! use mrc::ExtHeaderData;
//!
//! match reader.parse_extended_header() {
//!     ExtHeaderData::Fei1(records) => {
//!         println!("FEI1 tilt series ({} records)", records.len());
//!         for r in &records {
//!             println!("  tilt {:.1}°, defocus {:.1} µm",
//!                 r.alpha_tilt, r.defocus);
//!         }
//!     }
//!     ExtHeaderData::None => println!("No recognized extended header"),
//!     _ => {}
//! }
//! # Ok(()) }
//! ```
//!
//! Typed convenience methods: [`fei1_metadata`](Reader::fei1_metadata),
//! [`fei2_metadata`](Reader::fei2_metadata), [`ccp4_records`](Reader::ccp4_records),
//! [`mrco_records`](Reader::mrco_records), [`seri_records`](Reader::seri_records),
//! [`agar_records`](Reader::agar_records), [`imod_metadata`](Reader::imod_metadata).
//!
//! ## Validation
//!
//! [`validate_full`] checks header, file size, endianness, data
//! statistics, and NaN/Inf.  Returns a [`ValidationReport`]:
//!
//! ```no_run
//! use mrc::{validate_full, Severity};
//!
//! let report = mrc::validate_full("protein.mrc", false)?;
//! if !report.is_valid() {
//!     for issue in &report.issues {
//!         if issue.severity == Severity::Error {
//!             eprintln!("[{}] {}", issue.category, issue.message);
//!         }
//!     }
//! }
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```
//!
//! Use [`validate_reader`] if the file is already open.
//!
//! ## Error types
//!
//! Fallible functions return `Result<T, Error>`.  Match on variants:
//!
//! ```rust
//! use mrc::Error;
//!
//! fn describe(err: &Error) -> &str {
//!     match err {
//!         Error::Io(_) => "I/O failure",
//!         Error::InvalidHeader => "not a valid MRC file",
//!         Error::ModeMismatch { .. } => "wrong voxel type for this file",
//!         Error::BoundsError { .. } => "block outside volume",
//!         Error::FileSizeMismatch { .. } => "file truncated or has trailing data",
//!         _ => "other",
//!     }
//! }
//! ```
//!
//! [`HeaderValidationError`] gives fine-grained header diagnostics.
//!
//! ## Feature flags
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `mmap` | Memory-mapped readers and writers | ✅ |
//! | `f16` | Half-precision float via the `half` crate | ✅ |
//! | `simd` | AVX2 / NEON acceleration | ✅ |
//! | `parallel` | Parallel decode/convert/encode via rayon | ✅ |
//! | `gzip` | Gzip-compressed I/O | ✅ |
//! | `bzip2` | Bzip2-compressed I/O | ❌ |
//! | `ndarray` | Return volumes as `ndarray::Array3<T>` | ❌ |
//! | `serde` | Serialize/Deserialize support | ❌ |
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! # #[cfg(feature = "ndarray")]
//! # {
//! use ndarray::Array3;
//! let arr: Array3<f32> = reader.convert::<f32>().to_ndarray()?;
//! println!("{}×{}×{} array", arr.shape()[0], arr.shape()[1], arr.shape()[2]);
//! # }
//! # Ok(()) }
//! ```
//!
//! ## Endianness
//!
//! MRC files encode byte order via a 4-byte MACHST stamp at file offset 212.
//! [`FileEndian`] represents the detected byte order:
//!
//! ```rust
//! use mrc::FileEndian;
//! let stamp: [u8; 4] = [0x44, 0x44, 0x00, 0x00]; // little-endian
//! assert_eq!(FileEndian::from_machst(&stamp), FileEndian::LittleEndian);
//! ```
//!
//! New files are always little-endian.  The crate has a fallback: if the
//! MODE field is invalid under the detected endianness, the opposite
//! byte order is tried (handles files with a wrong MACHST stamp).
//!
//! ## Philosophy
//!
//! This crate does **one thing** — read and write MRC files.  It does no
//! array arithmetic, image processing, or type conversion beyond
//! MRC-specific shortcuts (`convert::<f32>()`, `slices_mode0`,
//! `slices_u8`).  Leave those to crates like `ndarray`, or your own code.

#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::perf)
)]
#![warn(missing_docs, clippy::cargo)]

mod engine;
mod error;
mod header;
mod io;
mod iter;
mod mode;
mod validate;

#[cfg(feature = "serde")]
mod serde_byte_array;

// Re-export core types
pub use engine::block::{VolumeShape, VoxelBlock};
/// Endianness of MRC file data.
pub use engine::endian::FileEndian;

// Re-export MRC-specific format utilities
pub use engine::convert::{convert_u8_slice_to_u16, convert_u16_slice_to_u8, reinterpret_m0};

pub use error::{Error, HeaderValidationError};
pub use header::{
    AGAR_RECORD_SIZE, AgarRecord, CCP4_RECORD_SIZE, Ccp4Record, ExtHeaderData, ExtHeaderType,
    FEI1_RECORD_SIZE, FEI2_RECORD_SIZE, Fei1Metadata, Fei2Metadata, Header, HeaderBuilder,
    ImodImageType, ImodInfo, ImodMetadata, MRCO_RECORD_SIZE, MrcoRecord, SERI_RECORD_SIZE,
    SeriRecord, parse_agar_records, parse_ccp4_records, parse_fei1_records, parse_fei2_records,
    parse_imod_metadata, parse_mrco_records, parse_seri_records,
};

pub use mode::{
    ComplexToRealStrategy, DataBlock, DataView, Float32Complex, Int16Complex, M0Interpretation,
    Mode, OwnedData, Voxel,
};

/// Half-precision floating point type (requires `f16` feature).
#[cfg(feature = "f16")]
pub use half::f16;
/// Consolidated MRC reader with automatic mmap/buffered backend selection.
pub use io::reader::Reader;

/// Auto-conversion wrapper returned by [`Reader::convert`].
pub use io::reader_common::ConvertReader;

/// MRC file writer and its builder.
pub use io::writer::{Writer, WriterBuilder};

/// Compression level for compressed MRC writers.
///
/// See [`WriterBuilder::compression`] for usage.
pub use io::writer::CompressionLevel;

/// Default decompression safety limit for gzip/bzip2 files (256 GiB).
///
/// Applied before the header is parsed, preventing decompression bombs.
/// Override via [`Reader::open_gzip_with_limit`] or
/// [`Reader::open_bzip2_with_limit`].
pub use io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES;

#[doc(hidden)]
pub use engine::codec::{decode_into, swap_bytes_in_place};

#[doc(hidden)]
pub use io::reader::{CompressionType, detect_compression};

/// Validation report types and comprehensive file checker.
pub use validate::{Severity, ValidationIssue, ValidationReport, validate_full, validate_reader};

/// Open an MRC file for reading, auto-detecting gzip or bzip2 compression.
///
/// This is a convenience wrapper around [`Reader::open`].
/// Common microscope quirks (NVERSION left at 0, `"MAP\0"` instead of `"MAP "`)
/// are handled transparently — no special flags needed.
///
/// For compressed files, decompression is capped at
/// [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB) to prevent bombs.
/// Use [`Reader::open_gzip_with_limit`] or [`Reader::open_bzip2_with_limit`]
/// to set a custom limit.
///
/// For permissive mode (returns `(Reader, Vec<String>)` instead of
/// `Reader`), or compressed-file-specific openers,
/// use [`Reader::open_permissive`], [`Reader::open_gzip`], etc. directly.
pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Reader, Error> {
    Reader::open(path)
}

/// Create a new MRC file for writing.
///
/// Returns a [`WriterBuilder`] that must be configured with at least
/// [`shape`](WriterBuilder::shape) and [`mode`](WriterBuilder::mode)
/// before calling [`finish`](WriterBuilder::finish) to open the file.
///
/// # Example
/// ```no_run
/// use mrc::create;
///
/// let mut writer = create("output.mrc")
///     .shape([256, 256, 128])
///     .mode::<f32>()
///     .finish()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn create<P: AsRef<std::path::Path>>(path: P) -> WriterBuilder {
    WriterBuilder::new(path)
}

/// Read an entire MRC volume into a `Vec<T>` with auto-mode detection.
///
/// This is a one-shot convenience over manually opening a [`Reader`] and
/// calling [`convert::<T>()`](Reader::convert) then [`read_volume`](Reader::read_volume).
/// The file can be in any MRC mode — the data is auto-converted to `T`.
///
/// Returns the parsed [`Header`] and the voxel data.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use mrc::read_as;
/// let (header, data): (_, Vec<f32>) = read_as("density.mrc")?;
/// println!("{}×{}×{} volume, {} voxels",
///     header.nx, header.ny, header.nz, data.len());
/// # Ok(()) }
/// ```
pub fn read_as<T: Voxel + crate::engine::convert::ConvertFrom<f32>, P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(Header, Vec<T>), Error> {
    let reader = Reader::open(path)?;
    let header = *reader.header();
    let volume = reader.convert::<T>().read_volume()?;
    Ok((header, volume.data))
}

/// Write an entire MRC volume from a `&[T]` with a single call.
///
/// Creates the file, writes the data, computes density statistics,
/// and finalizes — all in one step. The type `T` determines the
/// file's MRC mode (e.g. `f32` → Mode 2, `i16` → Mode 1).
///
/// # Examples
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use mrc::write_as;
/// let data = vec![0.0f32; 64 * 64 * 32];
/// write_as("output.mrc", &data, [64, 64, 32])?;
/// # Ok(()) }
/// ```
pub fn write_as<T: Voxel, P: AsRef<std::path::Path>>(
    path: P,
    data: &[T],
    shape: [usize; 3],
) -> Result<(), Error> {
    let mut writer = WriterBuilder::new(path).shape(shape).mode::<T>().finish()?;
    writer.set_data(data)?;
    writer.finalize()?;
    Ok(())
}
