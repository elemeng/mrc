//! Read and write MRC-2014 files — the standard in cryo-EM and structural
//! biology.
//!
//! This crate handles file I/O, byte-order detection, and type-safe data
//! access so you can focus on your science. It's fast (SIMD, parallel
//! encoding) and works with plain, gzip, and bzip2 files out of the box.
//!
//! # Quick example
//!
//! ```no_run
//! use mrc::{open, create, VoxelBlock};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let reader = open("protein.mrc")?;          // auto-detects compression
//!     for slice in reader.convert_slices::<f32>() {           // converts to f32 for you
//!         let block = slice?;
//!     }
//!
//!     let mut writer = create("output.mrc")
//!         .shape([512, 512, 256])
//!         .mode::<f32>()
//!         .finish()?;
//!     writer.write_block(&VoxelBlock::new(
//!         [0, 0, 0], [512, 512, 1],
//!         vec![0.0f32; 512 * 512],
//!     )?)?;
//!     writer.finalize()?;
//!     Ok(())
//! }
//! ```
//!
//! # Reading files
//!
//! Open any MRC file with [`open()`] or [`Reader::open`]. Compression is
//! detected from the file's magic bytes — no need to tell it gzip or bzip2.
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
//! * [`slices`](Reader::slices) — one Z-plane at a time
//! * [`slabs`](Reader::slabs) — batches of `k` Z-planes
//! * [`tiles`](Reader::tiles) — arbitrary 3D blocks
//! * [`subregion`](Reader::subregion) — a single block by coordinate
//! * [`convert_slices`](Reader::convert_slices) — any Z-plane, converted to any type
//!
//! Or grab the full volume in one call:
//!
//! * [`read_volume::<T>()`](Reader::read_volume) — full volume as any [`Voxel`] type
//! * [`convert_volume::<f32>()`](Reader::convert_volume) — full volume, any mode converted to `f32`
//! * [`convert_volume`](Reader::convert_volume) — full volume, converted to any type
//!
//! Each yields [`VoxelBlock<T>`] — a data chunk with its `offset` and
//! `shape`, so you always know where it belongs.
//!
//! For density maps stored as integers, use [`convert_slices`](Reader::convert_slices)
//! or [`convert_volume`](Reader::convert_volume) to get `f32` with
//! automatic mode conversion (no need to match the file's storage type):
//! It converts every MRC mode to `f32` (integer widening, complex→magnitude,
//! the works):
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! for slice in reader.convert_slices::<f32>() {
//!     let block = slice?;
//!     println!("slice {} mean density: {:.2}",
//!         block.offset[2],
//!         block.data.iter().sum::<f32>() / block.data.len() as f32);
//! }
//! # Ok(()) }
//! ```
//!
//! Or read the full volume at once with [`convert_volume::<f32>()`](Reader::convert_volume)
//! and wrap with `ndarray` for numpy-like slicing:
//!
//! ```text
//! let block = reader.convert_volume::<f32>()?;
//! let array = ndarray::Array3::from_shape_vec(
//!     [reader.shape().nz, reader.shape().ny, reader.shape().nx],
//!     block.data,
//! ).unwrap();
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
//! Common quirks from microscopes (NVERSION left at 0, `"MAP\0"` instead of
//! `"MAP "`) are handled transparently by [`open()`] — no special flags needed.
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
//!    (e.g. write `f32` data to a Float16 file).
//! 2. Optionally call [`update_header_stats`](Writer::update_header_stats)
//!    before finalize to fill in `dmin`/`dmax`/`dmean`/`rms`.
//! 3. **Finalize** with [`finalize`](Writer::finalize) to rewrite the header
//!    with final metadata. **Required** — without it the header is stale.
//!
//! Four backends through the same builder:
//!
//! | Backend | Builder method | Best for |
//! |---|---|---|
//! | [`Writer`] | [`finish()`](WriterBuilder::finish) | General use, writes straight to disk |
//! | [`MmapWriter`] | [`finish_mmap()`](WriterBuilder::finish_mmap) | Very large files (`mmap` feature) |
//! | [`GzipWriter`] | [`finish_gzip()`](WriterBuilder::finish_gzip) | Compressed output (`gzip` feature) |
//! | [`Bzip2Writer`] | [`finish_bzip2()`](WriterBuilder::finish_bzip2) | Compressed output (`bzip2` feature) |
//!
//! # Data modes
//!
//! MRC files encode voxels in one of several numeric modes. [`Mode`]
//! represents them at runtime; [`Voxel`] ties each Rust type to its mode
//! at compile time, catching mismatches before any data is read or written.
//!
//! | Mode | Rust type | Typical use |
//! |---|---|---|
//! | [`Int8`](Mode::Int8) (0) | `i8` | Binary masks |
//! | [`Int16`](Mode::Int16) (1) | `i16` | Raw cryo-EM density |
//! | [`Float32`](Mode::Float32) (2) | `f32` | Processed / reconstructed density |
//! | [`Uint16`](Mode::Uint16) (6) | `u16` | Segmentation labels |
//! | [`Float16`](Mode::Float16) (12) | `f16` | Half-precision storage (feature `f16`) |
//!
//! Packed 4-bit data ([`Mode::Packed4Bit`], mode 101) is handled transparently by
//! the unified API: [`convert_slices`](Reader::convert_slices) / [`convert_volume`](Reader::convert_volume)
//! unpack nibbles to `f32` or any target type, [`slices_u8`](Reader::slices_u8) / [`slabs_u8`](Reader::slabs_u8)
//! unpack to `u8` (0–15), and [`write_u4_block`](Writer::write_u4_block) packs `u8` values.
//!
//! When you don't know the mode ahead of time, use [`convert_slices`](Reader::convert_slices)
//! which converts any mode to the requested type (commonly `f32`).
//!
//! # Headers
//!
//! The [`Header`] struct mirrors the 1024-byte MRC-2014 fixed header.
//! Every field is a typed public field — dimensions, cell parameters,
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
//! # Philosophy
//!
//! This crate does **one thing** — read and write MRC files. It does not
//! do array arithmetic, image processing, or type conversion beyond a few
//! MRC-specific shortcuts (`convert_slices`, `slices_mode0`, `slices_u8`).
//! Leave those to crates like `ndarray`, or your own code.
//!
//! # Feature flags
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `mmap` | Memory-mapped readers and writers | ✅ |
//! | `f16` | Half-precision float via the `half` crate | ✅ |
//! | `simd` | AVX2 / NEON acceleration for integer→f32 | ✅ |
//! | `parallel` | Parallel encoding via `rayon` | ✅ |
//! | `gzip` | Gzip-compressed I/O | ✅ |
//! | `bzip2` | Bzip2-compressed I/O | ❌ |
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
//!   an Int16 file; use `convert_slices::<f32>()` instead
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
//! ## FEI extended headers
//!
//! Data from Thermo Fisher / FEI microscopes often carries FEI1 or FEI2
//! extended headers — one metadata record per image section.
//! [`Fei1Metadata`] and [`Fei2Metadata`] parse these into named fields
//! (dose, defocus, stage position, pixel size, magnification ...).
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::parse_fei1_records;
//! # let reader = mrc::Reader::open("tilt_series.mrc")?;
//! let bytes = reader.ext_header_bytes();
//! if let Some(records) = parse_fei1_records(bytes) {
//!     for r in &records {
//!         println!("tilt {:.1}°, defocus {:.1} µm", r.alpha_tilt, r.defocus);
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! ## File validation
//!
//! [`validate_full`](crate::validate::validate_full) runs comprehensive
//! checks on a file — header, size, endianness, data statistics (1 %
//! tolerance), and NaN / Inf scanning. Returns a
//! [`ValidationReport`](crate::validate::ValidationReport) with
//! categorised issues.
//!
//! If you already have an open [`Reader`], use
//! [`validate_reader`](crate::validate::validate_reader) to avoid
//! re-opening the file.
//!
//! # Real-world workflows
//!
//! ## 1. Process a tilt series from a microscope
//!
//! Files from Thermo Fisher / FEI microscopes often have quirks (NVERSION
//! left at 0, `"MAP\0"` instead of `"MAP "`). The crate handles these
//! transparently — [`open()`] works without special flags.
//!
//! A common pipeline: open a tilt series, read the FEI metadata (tilt
//! angles, defocus), then iterate over slices:
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
//! // Read FEI extended header metadata (tilt angles, defocus, etc.)
//! if let Some(records) = parse_fei1_records(reader.ext_header_bytes()) {
//!     for (i, r) in records.iter().enumerate() {
//!         println!("tilt {i}: α={:.1}°, defocus={:.1} µm",
//!             r.alpha_tilt, r.defocus);
//!     }
//! }
//!
//! // Process each slice
//! for slice in reader.convert_slices::<f32>() {
//!     let block = slice?;
//!     // block.data: Vec<f32> — ready for filtering, CTF correction, etc.
//! }
//! # Ok(()) }
//! ```
//!
//! If a file still fails to open, use
//! [`open_permissive`](Reader::open_permissive) for lenient header handling,
//! or [`validate_full`](crate::validate::validate_full) to diagnose the issue.
//!
//! ## 2. Write a processed map
//!
//! After processing, write the result as a new MRC file. Always call
//! [`finalize`](Writer::finalize) — without it the header is stale and the
//! file may be unreadable by other tools:
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
//! // ⚠️ Compute and store header density statistics
//! writer.update_header_stats()?;
//!
//! // ⚠️ REQUIRED: rewrites the header with final metadata
//! writer.finalize()?;
//! # Ok(()) }
//! ```
//!
//! **Without [`finalize`](Writer::finalize)** the header retains its initial
//! (zeroed or default) statistics and metadata. Most MRC readers will still
//! open the file, but density statistics will be wrong and tools like IMOD
//! or UCSF Chimera may display garbage contrast.
//!
//! ## 3. Read subtomogram averages from a volume stack
//!
//! Volume stacks (ISPG 401–630) pack multiple sub-volumes into one file,
//! each `mz` slices thick. Use [`volumes`](Reader::volumes) to iterate:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("averages.mrc")?;
//! for volume in reader.volumes::<f32>()? {
//!     let vol = volume?;
//!     println!("sub-volume at z={} ({}×{}×{} voxels)",
//!         vol.offset[2], vol.shape[0], vol.shape[1], vol.shape[2]);
//!     // vol.data: Vec<f32> with nx * ny * mz elements
//! }
//! # Ok(()) }
//! ```
//!
//! # Troubleshooting
//!
//! | Error | Likely cause | What to try |
//! |---|---|---|
//! | [`InvalidHeader`](Error::InvalidHeader) | Not an MRC file, or file has severe header corruption | Run `mrc-validate file.mrc` for diagnostics; try [`open_permissive`](Reader::open_permissive) |
//! | [`FileSizeMismatch`](Error::FileSizeMismatch) | File was truncated during transfer, or has trailing garbage | Re-download the file; check `mrc-validate` output |
//! | [`ModeMismatch`](Error::ModeMismatch) | Using `slices::<f32>()` on an Int16 file | Use [`convert_slices::<f32>()`](Reader::convert_slices) instead — it auto-converts any mode |
//! | [`BoundsError`](Error::BoundsError) | Requested block falls outside the volume | Check offset + shape against the volume dimensions |
//! | [`UnsupportedMode`](Error::UnsupportedMode) | File uses a mode this crate doesn't support (e.g. complex modes, or mode without the `f16` feature) | Enable the `f16` feature, or convert the file with another tool |
//! | Unexpected `Io` error | File permissions, filesystem issue, or path doesn't exist | Check the file path and permissions |
//! | File opens but values look wrong | Endianness mismatch or byte-order stamp is incorrect | The crate's endianness fallback handles most cases; try `mrc-validate` to confirm |

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod error;
mod fei;
mod header;
mod io;
mod iter;
mod mode;
pub mod validate;

// Re-export core types
pub use engine::block::{VolumeShape, VoxelBlock};
/// Endianness of MRC file data.
pub use engine::endian::FileEndian;

// Re-export MRC-specific format utilities
pub use engine::convert::{convert_u8_slice_to_u16, convert_u16_slice_to_u8, reinterpret_m0};

pub use error::{Error, HeaderValidationError};
pub use header::{Header, HeaderBuilder};
pub use mode::{
    ComplexToRealStrategy, Float32Complex, Int16Complex, M0Interpretation, Mode, Voxel,
};

/// Half-precision floating point type (requires `f16` feature).
#[cfg(feature = "f16")]
pub use half::f16;
/// Buffered MRC reader with lazy slice/slab iterators.
pub use io::buffered::Reader;

/// MRC file writer and its builder.
pub use io::writer::{Writer, WriterBuilder};
/// Lazy iterator over MRC voxel blocks.
pub use iter::RegionIter;
/// Stepping strategies for [`RegionIter`].
pub use iter::{SlabStepper, SliceStepper, TileStepper};

/// Memory-mapped MRC writer (requires `mmap` feature).
#[cfg(feature = "mmap")]
pub use io::writer::MmapWriter;

/// Memory-mapped MRC reader (requires `mmap` feature).
#[cfg(feature = "mmap")]
pub use io::mmap_reader::MmapReader;

/// Gzip-compressed MRC writer (requires `gzip` feature).
#[cfg(feature = "gzip")]
pub use io::gzip::GzipWriter;

/// Bzip2-compressed MRC writer (requires `bzip2` feature).
#[cfg(feature = "bzip2")]
pub use io::bzip2::Bzip2Writer;

/// FEI extended header metadata types and parsers.
pub use fei::{
    FEI1_RECORD_SIZE, FEI2_RECORD_SIZE, Fei1Metadata, Fei2Metadata, parse_fei1_records,
    parse_fei2_records,
};

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

#[cfg(test)]
mod integration_tests {
    use crate::*;

    /// Create a unique temp file path that is automatically deleted on drop.
    struct TempMrc(std::path::PathBuf);

    impl TempMrc {
        fn new(suffix: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("mrc_test_{}_{}.mrc", std::process::id(), suffix));
            // Remove any leftover from a previous run
            let _ = std::fs::remove_file(&p);
            Self(p)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempMrc {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// Write Float32 volume, read it back byte-for-byte.
    #[test]
    fn roundtrip_f32() {
        let f = TempMrc::new("f32");
        let nx = 16;
        let ny = 8;
        let nz = 4;

        let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume::<f32>().unwrap();
        assert_eq!(block.data, data);
        assert_eq!(&block.offset, &[0, 0, 0]);
        assert_eq!(&block.shape, &[nx, ny, nz]);

        // convert_volume::<f32> on Float32 should give the same result
        let block2 = r.convert_volume::<f32>().unwrap();
        assert_eq!(block2.data, data);
    }

    /// Write Int16, read back via convert_volume::<f32> (auto-conversion).
    #[test]
    fn roundtrip_i16_to_f32() {
        let f = TempMrc::new("i16");
        let nx = 16;
        let ny = 8;
        let nz = 4;
        let total = nx * ny * nz;

        let src: Vec<i16> = (0..total).map(|i| (i as i16) - 100).collect();
        let expected_f32: Vec<f32> = src.iter().map(|&v| v as f32).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode::<i16>()
                .finish()
                .unwrap();
            w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        // convert_volume::<f32> auto-converts Int16 → f32
        let block = r.convert_volume::<f32>().unwrap();
        assert_eq!(block.data, expected_f32);

        // convert_slices::<f32> should also match
        let all: Vec<f32> = r
            .convert_slices::<f32>()
            .flat_map(|s| s.unwrap().data)
            .collect();
        assert_eq!(all, expected_f32);
    }

    /// Write Uint16, read back via read_volume::<u16>().
    #[test]
    fn roundtrip_u16() {
        let f = TempMrc::new("u16");
        let nx = 8;
        let ny = 8;
        let nz = 2;

        let src: Vec<u16> = (0..nx * ny * nz).map(|i| (i * 2) as u16).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode::<u16>()
                .finish()
                .unwrap();
            w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume::<u16>().unwrap();
        assert_eq!(block.data, src);
    }

    /// Write multiple slabs, read back with subregion.
    #[test]
    fn roundtrip_subregion() {
        let f = TempMrc::new("subregion");
        let nx = 32;
        let ny = 32;
        let nz = 8;

        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        for z in 0..nz {
            let slice = vec![(z * nx * ny) as f32; nx * ny];
            w.write_block(&VoxelBlock::new([0, 0, z], [nx, ny, 1], slice).unwrap())
                .unwrap();
        }
        w.finalize().unwrap();

        let r = Reader::open(f.path()).unwrap();
        // Read a middle subregion
        let block = r.subregion::<f32>([4, 4, 2], [8, 8, 3]).unwrap();
        assert_eq!(block.offset, [4, 4, 2]);
        assert_eq!(block.shape, [8, 8, 3]);
        assert_eq!(block.data.len(), 8 * 8 * 3);
    }

    /// Read entire volume via read_volume matches collecting convert_slices::<f32>.
    #[test]
    fn read_volume_via_convert_slices_f32() {
        let f = TempMrc::new("vol_vs_slices");
        let nx = 10;
        let ny = 10;
        let nz = 5;

        let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        let vol = r.read_volume::<f32>().unwrap();
        let collected: Vec<f32> = r
            .convert_slices::<f32>()
            .flat_map(|s| s.unwrap().data)
            .collect();
        assert_eq!(vol.data, collected);
    }

    /// Gzip compressed roundtrip.
    #[cfg(feature = "gzip")]
    #[test]
    fn roundtrip_gzip() {
        let f = TempMrc::new("gzip");
        let nx = 8;
        let ny = 8;
        let nz = 4;

        let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode::<f32>()
                .finish_gzip()
                .unwrap();
            w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        // Reader::open auto-detects gzip
        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume::<f32>().unwrap();
        assert_eq!(block.data, data);
    }

    /// Header statistics roundtrip: write data, update stats, read back.
    #[test]
    fn update_header_stats_roundtrip() {
        let f = TempMrc::new("stats");
        let nx = 4;
        let ny = 4;
        let nz = 2;
        let total = nx * ny * nz;

        let data: Vec<f32> = (0..total).map(|i| i as f32).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
                .unwrap();
            w.update_header_stats().unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        assert!(r.validate_header_stats().is_ok());
    }

    // ── Packed4Bit (Mode 101) tests ──────────────────────────────────────

    /// Write Mode 101 with even width, read back via read_volume_u8.
    #[test]
    fn mode101_roundtrip_even() {
        let f = TempMrc::new("m101_even");
        let nx = 4;
        let ny = 4;
        let nz = 2;
        let total = nx * ny * nz;

        let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode_raw(101)
                .finish()
                .unwrap();
            w.write_u4_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume_u8().unwrap();
        assert_eq!(block.data, src);

        // slices_u8 should also match
        let collected: Vec<u8> = r.slices_u8().flat_map(|s| s.unwrap().data).collect();
        assert_eq!(collected, src);
    }

    /// Write Mode 101 with odd width (nx=3), read back.
    #[test]
    fn mode101_roundtrip_odd() {
        let f = TempMrc::new("m101_odd");
        let nx = 3;
        let ny = 2;
        let nz = 1;
        let total = nx * ny * nz;

        let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode_raw(101)
                .finish()
                .unwrap();
            w.write_u4_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume_u8().unwrap();
        assert_eq!(block.data, src);
    }

    /// Header stats on a Mode 101 file.
    #[test]
    fn mode101_header_stats() {
        let f = TempMrc::new("m101_stats");
        let nx = 8;
        let ny = 4;
        let nz = 1;
        let total = nx * ny * nz;

        let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
        {
            let mut w = create(f.path())
                .shape([nx, ny, nz])
                .mode_raw(101)
                .finish()
                .unwrap();
            w.write_u4_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
                .unwrap();
            w.update_header_stats().unwrap();
            w.finalize().unwrap();
        }

        let r = Reader::open(f.path()).unwrap();
        assert!(r.validate_header_stats().is_ok());
    }

    /// Value > 15 should produce an error.
    #[test]
    fn mode101_value_overflow() {
        let f = TempMrc::new("m101_overflow");
        let src = vec![16u8]; // 16 > 15
        let mut w = create(f.path())
            .shape([1, 1, 1])
            .mode_raw(101)
            .finish()
            .unwrap();
        let result = w.write_u4_block(&VoxelBlock::new([0, 0, 0], [1, 1, 1], src).unwrap());
        assert!(result.is_err());
    }
}
