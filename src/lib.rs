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
//!     for slice in reader.slices_f32()? {
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
//!     ))?;
//!     writer.finalize()?;
//!     Ok(())
//! }
//! ```

#![cfg_attr(feature = "f16", feature(f16))]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod any_reader;
mod error;
mod header;
mod iter;
mod mode;
mod reader;
mod reader_common;
mod writer;

#[cfg(feature = "mmap")]
mod mmap_reader;

#[cfg(feature = "gzip")]
mod gzip;

#[cfg(feature = "bzip2")]
mod bzip2;

mod fei;

mod engine;

// Re-export core types
pub use engine::block::{VolumeShape, VoxelBlock};
pub use engine::endian::FileEndian;

// Re-export MRC-specific format utilities
pub use engine::convert::{
    convert_u16_slice_to_u8, convert_u8_slice_to_u16, reinterpret_m0,
};

pub use error::{Error, HeaderValidationError};
pub use header::{Header, HeaderBuilder};
pub use mode::{
    ComplexToRealStrategy, Float32Complex, Int16Complex, M0Interpretation, Mode, Packed4Bit, Voxel,
};
pub use reader::Reader;
pub use iter::{BlockIter, SliceIter, SlabIter};
pub use writer::{Writer, WriterBuilder};

#[cfg(feature = "mmap")]
pub use writer::{MmapWriter, MmapWriterBuilder};

#[cfg(feature = "mmap")]
pub use mmap_reader::MmapReader;

#[cfg(feature = "gzip")]
pub use gzip::{GzipReader, GzipWriter};

#[cfg(feature = "bzip2")]
pub use bzip2::{Bzip2Reader, Bzip2Writer};

pub use fei::{Fei1Metadata, Fei2Metadata, parse_fei1_records, parse_fei2_records, FEI1_RECORD_SIZE, FEI2_RECORD_SIZE};

pub use any_reader::{CompressionType, MrcReader, detect_compression};

/// Iterator over slices yielding `f32` voxel blocks.
pub type SliceIterF32<'a> =
    Box<dyn Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + 'a>;

/// Open an MRC file for reading, auto-detecting gzip or bzip2 compression.
pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<MrcReader, Error> {
    MrcReader::open(path)
}

/// Create a new MRC file for writing.
pub fn create<P: AsRef<std::path::Path>>(path: P) -> WriterBuilder {
    WriterBuilder::new(path)
}

/// Open an MRC file via memory mapping (requires the `mmap` feature).
#[cfg(feature = "mmap")]
pub fn open_mmap<P: AsRef<std::path::Path>>(path: P) -> Result<MmapReader, Error> {
    MmapReader::open(path)
}

/// Create a new memory-mapped MRC file for writing (requires the `mmap` feature).
#[cfg(feature = "mmap")]
pub fn create_mmap<P: AsRef<std::path::Path>>(path: P) -> MmapWriterBuilder {
    MmapWriterBuilder::new(path)
}
