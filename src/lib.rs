//! MRC file format library for cryo-EM and tomography
//!
//! Provides high-performance reading and writing of MRC-2014 files.
//!
//! # Design Philosophy
//!
//! This crate focuses on a single responsibility: **correctly reading and writing
//! MRC files**. Type conversion is the caller's responsibility, with only a small
//! set of MRC-specific conveniences (e.g. `slices_f32` for the common i16→f32 case).
//!
//! # Quick Example
//!
//! ```no_run
//! use mrc::{open, create, VoxelBlock};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Reading (auto-detects gzip/bzip2)
//!     let reader = open("protein.mrc")?;
//!     for slice in reader.slices_f32() {
//!         let block = slice?;
//!         // block.data is Vec<f32>
//!     }
//!
//!     // Writing
//!     let mut writer = create("output.mrc")
//!         .shape([512, 512, 256])
//!         .mode::<f32>()
//!         .finish()?;
//!     writer.write_block(&VoxelBlock::new(
//!         [0, 0, 0],
//!         [512, 512, 1],
//!         vec![0.0f32; 512 * 512],
//!     )?)?;
//!     writer.finalize()?;
//!     Ok(())
//! }
//! ```

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod error;
mod fei;
mod header;
mod io;
mod iter;
mod mode;

// Re-export core types
pub use engine::block::{VolumeShape, VoxelBlock};
/// Endianness of MRC file data.
pub use engine::endian::FileEndian;

// Re-export MRC-specific format utilities
pub use engine::convert::{convert_u8_slice_to_u16, convert_u16_slice_to_u8, reinterpret_m0};

pub use error::{Error, HeaderValidationError};
pub use header::{Header, HeaderBuilder};
pub use mode::{
    ComplexToRealStrategy, Float32Complex, Int16Complex, M0Interpretation, Mode, Packed4Bit, Voxel,
};

/// Half-precision floating point type (requires `f16` feature).
#[cfg(feature = "f16")]
pub use half::f16;
/// Buffered MRC reader with lazy slice/slab iterators.
pub use io::buffered::Reader;
#[doc(hidden)]
pub use io::reader_common::ReaderExt;
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

#[doc(hidden)]
pub use io::reader::{CompressionType, detect_compression};

/// Open an MRC file for reading, auto-detecting gzip or bzip2 compression.
pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Reader, Error> {
    Reader::open(path)
}

/// Create a new MRC file for writing.
pub fn create<P: AsRef<std::path::Path>>(path: P) -> WriterBuilder {
    WriterBuilder::new(path)
}


