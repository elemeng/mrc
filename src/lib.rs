//! Read and write MRC-2014 files — the standard in cryo-EM and structural
//! biology.
//!
//! This crate handles file I/O, byte-order detection, and type-safe data
//! access so you can focus on your science. It's fast (SIMD, parallel
//! encoding) and works with plain, gzip, and (optionally) bzip2 files.
//!
//! See the [README](https://github.com/elemeng/mrc#readme) for installation
//! instructions, CLI tools, and the project roadmap.
//!
//! # Quick example
//!
//! ```no_run
//! use mrc::{open, create, VoxelBlock};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Read — auto-detects gzip/bzip2 compression
//! let reader = open("density.mrc")?;
//! for slice in reader.convert::<f32>().slices() {
//!     let _block = slice?; // VoxelBlock<f32>
//! }
//!
//! // Write
//! let mut writer = create("output.mrc")
//!     .shape([512, 512, 256])
//!     .mode::<f32>()
//!     .finish()?;
//! writer.write_block(&VoxelBlock::new(
//!     [0, 0, 0], [512, 512, 1],
//!     vec![0.0f32; 512 * 512],
//! )?)?;
//! writer.finalize()?;
//! # Ok(()) }
//! ```
//!
//! # Reading files
//!
//! Open any MRC file with [`open()`] or [`Reader::open`]. Compression is
//! detected from magic bytes — no need to hint gzip or bzip2.
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::Reader;
//! let reader = Reader::open("tilt_series.mrc")?;
//! println!("{}×{}×{} voxels, mode {:?}",
//!     reader.shape().nx, reader.shape().ny, reader.shape().nz,
//!     reader.mode());
//! # Ok(()) }
//! ```
//!
//! Then pick an iteration method:
//!
//! * [`slices`](ReaderMethods::slices) — one Z-plane at a time
//! * [`slabs`](ReaderMethods::slabs) — batches of `k` Z-planes
//! * [`tiles`](ReaderMethods::tiles) — arbitrary 3D blocks
//! * [`subregion`](ReaderMethods::subregion) — a single block by coordinate
//!
//! > **Trait imports (optional):** Iterator and conversion methods are
//! > available as inherent methods on `Reader` and `MmapReader` without any
//! > import. The [`ReaderMethods`] and [`ConvertMethods`] traits are also
//! > re-exported for advanced use (e.g. generic code over reader types).
//!
//! For automatic mode conversion, use [`convert`](ConvertMethods::convert):
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! for slice in reader.convert::<f32>().slices() {
//!     let block = slice?;
//!     println!("slice {} mean density: {:.2}",
//!         block.offset[2],
//!         block.data.iter().sum::<f32>() / block.data.len() as f32);
//! }
//! # Ok(()) }
//! ```
//!
//! Or read the full volume in one call:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! let block = reader.convert::<f32>().read_volume()?;
//! println!("read {} voxels", block.data.len());
//! # Ok(()) }
//! ```
//!
//! When the `ndarray` feature is enabled, get numpy-like multidimensional access:
//!
//! ```no_run
//! # #[cfg(feature = "ndarray")] {
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! let arr = reader.to_ndarray::<f32>()?;
//! // arr is ndarray::Array3<f32> with shape [nz, ny, nx]
//! let center = arr[[arr.shape()[0] / 2, arr.shape()[1] / 2, arr.shape()[2] / 2]];
//! # Ok(()) }
//! # }
//! ```
//!
//! ### Reading from memory or streams
//!
//! When your data is already in memory (e.g. from a camera readout, network
//! stream, or embedded resource), use [`Reader::from_reader`] or
//! [`Reader::from_bytes`]:
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
//! ### Large files
//!
//! When the file does not fit in RAM, use [`MmapReader`] (requires the
//! `mmap` feature). Same iterator API, zero-copy [`slab_as`](MmapReader::slab_as),
//! OS-managed paging.
//!
//! ### Quirky files
//!
//! Common microscope quirks (NVERSION left at 0, `"MAP\0"` instead of `"MAP "`)
//! are handled transparently by [`open()`] — no special flags needed.
//!
//! For esoteric or severely non-standard files, use
//! [`Reader::open_permissive`] which turns non-critical header issues into
//! warnings instead of hard errors:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # use mrc::Reader;
//! let (reader, warnings) = Reader::open_permissive("legacy.mrc")?;
//! for w in &warnings { eprintln!("note: {w}"); }
//! # Ok(()) }
//! ```
//!
//! # Writing files
//!
//! Use [`create()`] to get a [`WriterBuilder`], set the shape and voxel type,
//! then call [`finish`](WriterBuilder::finish).
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::create;
//! let mut writer = create("output.mrc")
//!     .shape([256, 256, 128])
//!     .mode::<f32>()
//!     .finish()?;
//! # Ok(()) }
//! ```
//!
//! The lifecycle:
//!
//! 1. **Write** blocks with [`write_block`](Writer::write_block). The type
//!    `T` matches the file's mode — a compile-time check that prevents
//!    accidentally treating bytes as the wrong kind of number.
//!    Use [`write_block_as`](Writer::write_block_as) for automatic conversion
//!    (e.g. write `f32` data to an Int16 or Float16 file).
//! 2. Optionally call [`update_header_stats`](Writer::update_header_stats)
//!    to fill in `dmin`/`dmax`/`dmean`/`rms`.
//! 3. **Finalize** with [`finalize`](Writer::finalize) to rewrite the header
//!    with final metadata. **Required** — without it the header is stale.
//!
//! Four backends through the same builder, plus in-memory output:
//!
//! | Backend | Builder method | Best for |
//! |---|---|---|
//! | [`Writer`] | [`finish()`](WriterBuilder::finish) | General use, writes straight to disk |
//! | [`Writer`] (in-memory) | [`Writer::from_writer`] | Memory buffer, e.g. `Cursor<Vec<u8>>` |
//! | [`MmapWriter`] | [`finish_mmap()`](WriterBuilder::finish_mmap) | Very large files (`mmap` feature) |
//! | [`GzipWriter`] | [`finish_gzip()`](WriterBuilder::finish_gzip) | Compressed output (`gzip` feature) |
//! | [`Bzip2Writer`] | [`finish_bzip2()`](WriterBuilder::finish_bzip2) | Compressed output (`bzip2` feature) |
//!
//! Compressed writers support configurable [`Compression`] level via
//! [`WriterBuilder::compression`]:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::{Compression, create};
//!
//! let mut writer = create("output.mrc.gz")
//!     .shape([256, 256, 128])
//!     .mode::<f32>()
//!     .compression(Compression::Best)
//!     .finish_gzip()?;
//! # Ok(()) }
//! ```
//!
//! # Data modes
//!
//! MRC files encode voxels in one of several numeric modes. [`Mode`]
//! represents them at runtime; [`Voxel`] ties each Rust type to its mode
//! at compile time, catching mismatches before any data flows.
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
//! | [`Packed4Bit`](Mode::Packed4Bit) (101) | `u8` via [`slices_u8`](ReaderMethods::slices_u8) | 4-bit packed data; no `Voxel` impl |
//!
//! Packed 4-bit data is handled transparently by the unified API:
//! [`convert::<f32>()`](ConvertMethods::convert) unpacks nibbles to `f32`,
//! [`slices_u8`](ReaderMethods::slices_u8) / [`slabs_u8`](ReaderMethods::slabs_u8) unpack
//! to `u8` (0–15), and [`write_u4_block`](Writer::write_u4_block) packs
//! `u8` values back.
//!
//! When you don't know the mode ahead of time, use
//! [`convert::<f32>()`](ConvertMethods::convert) which converts any mode to `f32`.
//!
//! # Headers
//!
//! The [`Header`] struct mirrors the 1024-byte MRC-2014 fixed header.
//! Every field is a typed public member — dimensions, cell parameters,
//! axis mapping, density statistics, text labels, and more.
//!
//! ```
//! use mrc::Header;
//! let h = Header::new();
//! assert_eq!(h.map, *b"MAP ");
//! ```
//!
//! For fluent construction with validation, use [`HeaderBuilder`]:
//!
//! ```
//! use mrc::HeaderBuilder;
//! let header = HeaderBuilder::new()
//!     .shape([512, 512, 256])
//!     .mode::<f32>()
//!     .cell_lengths(1.0, 1.0, 1.0)
//!     .cell_angles(90.0, 90.0, 90.0)
//!     .origin([0.0, 0.0, 0.0])
//!     .nstart([0, 0, 0])
//!     .sampling([512, 512, 256])
//!     .add_label("reconstructed map")
//!     .build()?;
//! # Ok::<_, mrc::HeaderValidationError>(())
//! ```
//!
//! Three validation levels:
//!
//! * [`validate`](Header::validate) — quick yes / no
//! * [`validate_detailed`](Header::validate_detailed) — tells you exactly
//!   what is wrong via [`HeaderValidationError`]
//! * [`validate_permissive`](Header::validate_permissive) — warnings for
//!   non-critical issues
//!
//! ### Convenience API
//!
//! The [`Header`] provides computed properties for common queries:
//!
//! ```rust
//! use mrc::Header;
//! let h = Header::new();
//! let vol = h.cell_volume();      // unit cell volume in Å³
//! let (dmin, dmax, dmean, rms) = h.density_stats();
//! let sampling = h.sampling();    // [mx, my, mz]
//! let label = h.label_at(0);      // first label, or None
//! assert!(h.is_standard_map());   // MAP field is "MAP "
//! ```
//!
//! ### Manual header parsing
//!
//! Decode a raw 1024-byte header block with automatic endianness detection:
//!
//! ```rust
//! use mrc::Header;
//! let raw = [0u8; 1024];
//! // ... fill raw bytes from file ...
//! let header = Header::decode_from_bytes(&raw);
//! ```
//!
//! When the MACHST byte-order stamp is wrong (common in some EPU files),
//! the decoder tries the opposite endianness automatically:
//!
//! ```rust
//! # use mrc::Header;
//! # let raw = [0u8; 1024];
//! let (header, warning) = Header::decode_from_bytes_with_info(&raw);
//! if let Some(w) = warning {
//!     eprintln!("byte-order fallback used: {w}");
//! }
//! ```
//!
//! # Extended headers
//!
//! Many MRC files carry additional metadata after the 1024-byte fixed header
//! in an **extended header** region. The type is identified by the 4-byte
//! `exttyp` field in the header's `extra[8..12]`.
//!
//! The [`ExtHeaderType`] enum identifies the format without parsing:
//!
//! ```rust
//! use mrc::{Header, ExtHeaderType};
//! let header = Header::new();
//! match ExtHeaderType::from_header(&header) {
//!     ExtHeaderType::Fei1 => println!("FEI Type 1"),
//!     ExtHeaderType::Ccp4 => println!("CCP4"),
//!     ExtHeaderType::Unknown(id) => {
//!         println!("Unknown: {:?}", std::str::from_utf8(&id));
//!     }
//!     _ => {}
//! }
//! ```
//!
//! Instead of calling individual parser functions, use the auto-dispatch
//! method on any open reader:
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
//!     ExtHeaderData::Ccp4(records) => {
//!         println!("CCP4 symmetry ({} records)", records.len());
//!     }
//!     ExtHeaderData::Seri(records) => {
//!         println!("  first tilt: {:.1}°", records[0].alpha_tilt);
//!     }
//!     ExtHeaderData::None => println!("No recognized extended header"),
//!     _ => {}
//! }
//! # Ok(()) }
//! ```
//!
//! Typed convenience methods give direct access without pattern matching:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("file.mrc")?;
//! if let Some(records) = reader.fei1_metadata() {
//!     println!("{} FEI1 records", records.len());
//! }
//! if let Some(imod) = reader.imod_metadata() {
//!     println!("IMOD type {:?}, tilt increment {:.1}°",
//!         imod.image_type, imod.tilt_increment);
//! }
//! # Ok(()) }
//! ```
//!
//! Available: [`fei1_metadata`](crate::Reader::fei1_metadata),
//! [`fei2_metadata`](crate::Reader::fei2_metadata),
//! [`ccp4_records`](crate::Reader::ccp4_records),
//! [`mrco_records`](crate::Reader::mrco_records),
//! [`seri_records`](crate::Reader::seri_records),
//! [`agar_records`](crate::Reader::agar_records),
//! [`imod_metadata`](crate::Reader::imod_metadata).
//!
//! # Feature flags
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `mmap` | Memory-mapped readers and writers | ✅ |
//! | `f16` | Half-precision float via the `half` crate | ✅ |
//! | `simd` | AVX2 / NEON acceleration for integer→f32, f16↔f32, byte-swap, stats | ✅ |
//! | `parallel` | Parallel encoding via `rayon` | ✅ |
//! | `gzip` | Gzip-compressed I/O | ✅ |
//! | `bzip2` | Bzip2-compressed I/O | ❌ |
//! | `ndarray` | Return volumes as `ndarray::Array3<T>` via `to_ndarray()` | ❌ |
//! | `serde` | Serialize/Deserialize support via `serde` | ❌ |
//!
//! # Advanced topics
//!
//! ## Error handling
//!
//! Fallible functions return `Result<T, Error>`. The errors you will
//! actually hit in practice:
//!
//! * [`Io`](Error::Io) — the file could not be read or written
//! * [`InvalidHeader`](Error::InvalidHeader) — not a valid MRC file
//! * [`ModeMismatch`](Error::ModeMismatch) — calling `slices::<f32>()` on
//!   an Int16 file; use `convert::<f32>()` instead
//! * [`BoundsError`](Error::BoundsError) — read or write outside the volume
//! * [`FileSizeMismatch`](Error::FileSizeMismatch) — file truncated or
//!   has trailing garbage
//!
//! [`HeaderValidationError`] gives fine-grained diagnostics for header
//! problems (bad dimensions, wrong MAP field, invalid NVERSION ...).
//!
//! ## Endianness
//!
//! MRC files encode byte order via a 4-byte MACHST stamp. [`FileEndian`]
//! handles detection and conversion automatically. New files are always
//! little-endian, matching modern hardware and the Python `mrcfile` library.
//!
//! The crate has a fallback: if the MODE field is invalid under the detected
//! endianness, the opposite byte order is tried. This handles files with a
//! wrong MACHST stamp but correct data.
//!
//! ## Compression auto-detection
//!
//! [`Reader::open`] reads the first two bytes of the file:
//!
//! | Magic bytes | Format |
//! |---|---|
//! | `\x1f\x8b` | Gzip |
//! | `BZ` | Bzip2 |
//! | anything else | Plain |
//!
//! Plain MRC files are memory-mapped or buffered directly. Compressed files
//! are fully decompressed into memory on open, with a hard cap of
//! [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB) to prevent bombs.
//! Use [`Reader::open_gzip_with_limit`] or
//! [`Reader::open_bzip2_with_limit`] for a custom limit.
//!
//! > **Large compressed files:** If the uncompressed data exceeds available RAM,
//! > decompress with `gunzip` or `bunzip2` first, then use [`MmapReader`] for
//! > zero-copy access — the OS pages data on demand without loading the whole
//! > file into memory.
//!
//! ## File validation
//!
//! [`validate_full`](validate::validate_full) runs comprehensive checks
//! on a file — header, size, endianness, data statistics (1% tolerance),
//! and NaN / Inf scanning. Returns a
//! [`ValidationReport`](validate::ValidationReport) with categorized issues.
//!
//! If you already have an open [`Reader`], use
//! [`validate_reader`](validate::validate_reader) to avoid re-opening
//! the file.
//!
//! # Real-world workflows
//!
//! ## 1. Process a tilt series
//!
//! A common cryo-EM workflow: open a tilt series, read the FEI metadata,
//! then iterate over slices:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::{open, parse_fei1_records};
//!
//! let reader = open("tiltseries.mrc")?;
//! println!("{}×{}×{} voxels, mode {:?}",
//!     reader.shape().nx, reader.shape().ny, reader.shape().nz,
//!     reader.mode());
//!
//! // Read FEI extended header metadata
//! if let Some(records) = parse_fei1_records(reader.ext_header_bytes()) {
//!     for (i, r) in records.iter().enumerate() {
//!         println!("tilt {i}: α={:.1}°, defocus={:.1} µm",
//!             r.alpha_tilt, r.defocus);
//!     }
//! }
//!
//! // Process each slice
//! for slice in reader.convert::<f32>().slices() {
//!     let block = slice?;
//!     // block.data: Vec<f32> — ready for filtering, CTF correction, etc.
//! }
//! # Ok(()) }
//! ```
//!
//! If a file fails to open, try [`open_permissive`](Reader::open_permissive)
//! for lenient header handling, or [`validate_full`](validate::validate_full)
//! to diagnose the issue.
//!
//! ## 2. Write a processed map
//!
//! Always call [`finalize`](Writer::finalize) — without it the header is
//! stale and density statistics will be wrong (tools display wrong contrast).
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::create;
//!
//! let mut writer = create("reconstructed.mrc")
//!     .shape([512, 512, 256])
//!     .mode::<f32>()
//!     .finish()?;
//!
//! for z in 0..256 {
//!     let slice = vec![0.0f32; 512 * 512];
//!     writer.write_block(&mrc::VoxelBlock::new(
//!         [0, 0, z], [512, 512, 1], slice,
//!     )?)?;
//! }
//!
//! writer.update_header_stats()?;
//! writer.finalize()?;
//! # Ok(()) }
//! ```
//!
//! ## 3. Read subtomogram averages from a volume stack
//!
//! Volume stacks (ISPG 401–630) pack multiple sub-volumes into one file,
//! each `mz` slices thick. Use [`volumes`](ReaderMethods::volumes) to iterate:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("averages.mrc")?;
//! for volume in reader.volumes::<f32>()? {
//!     let vol = volume?;
//!     println!("sub-volume at z={} ({}×{}×{} voxels)",
//!         vol.offset[2], vol.shape[0], vol.shape[1], vol.shape[2]);
//! }
//! # Ok(()) }
//! ```
//!
//! # Troubleshooting
//!
//! | Error | Likely cause | What to try |
//! |---|---|---|
//! | [`InvalidHeader`](Error::InvalidHeader) | Not an MRC file, or header corruption | Run `mrc-validate file.mrc`; try [`open_permissive`](Reader::open_permissive) |
//! | [`FileSizeMismatch`](Error::FileSizeMismatch) | File truncated or has trailing garbage | Re-download or check `mrc-validate` output |
//! | [`ModeMismatch`](Error::ModeMismatch) | Using `slices::<f32>()` on an Int16 file | Use [`convert::<f32>()`](ConvertMethods::convert) — auto-converts any mode |
//! | [`BoundsError`](Error::BoundsError) | Block outside volume | Check offset + shape against dimensions |
//! | [`UnsupportedMode`](Error::UnsupportedMode) | Unrecognized mode, or mode needs the `f16` feature | Enable `f16` feature or convert with another tool |
//! | `Io` error | File permissions, filesystem issue | Check the file path and permissions |
//! | Values look wrong | Endianness mismatch | The endianness fallback handles most cases; try `mrc-validate` |
//!
//! # Philosophy
//!
//! This crate does **one thing** — read and write MRC files. It does no array
//! arithmetic, image processing, or type conversion beyond MRC-specific
//! shortcuts (`convert::<f32>()`, `slices_mode0`, `slices_u8`).
//! Leave those to crates like `ndarray`, or your own code.

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
pub mod validate;

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
    ComplexToRealStrategy, Float32Complex, Int16Complex, M0Interpretation, Mode, Voxel,
};

/// Half-precision floating point type (requires `f16` feature).
#[cfg(feature = "f16")]
pub use half::f16;
/// Consolidated MRC reader with automatic mmap/buffered backend selection.
pub use io::reader::Reader;

/// MRC file writer and its builder.
pub use io::writer::{Writer, WriterBuilder};
/// Lazy iterator over MRC voxel blocks.
pub use iter::RegionIter;
/// Stepping strategies for [`RegionIter`].
pub use iter::{SlabStepper, SliceStepper, TileStepper};

/// Compression level for compressed MRC writers.
///
/// See [`WriterBuilder::compression`] for usage.
pub use io::writer::Compression;

/// Gzip-compressed MRC writer (requires `gzip` feature).
#[cfg(feature = "gzip")]
pub use io::gzip::GzipWriter;

/// Bzip2-compressed MRC writer (requires `bzip2` feature).
#[cfg(feature = "bzip2")]
pub use io::bzip2::Bzip2Writer;

/// Default decompression safety limit for gzip/bzip2 files (256 GiB).
///
/// Applied before the header is parsed, preventing decompression bombs.
/// Override via [`Reader::open_gzip_with_limit`] or
/// [`Reader::open_bzip2_with_limit`].
pub use io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES;

#[doc(hidden)]
pub use io::reader::{CompressionType, detect_compression};

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
/// For permissive mode or compressed-file-specific openers,
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
