//! MRC file format library for cryo-EM and tomography
//!
//! Provides high-performance reading and writing of MRC-2014 files.
//!
//! # Design Philosophy
//!
//! This crate focuses on a single responsibility: **correctly reading and writing
//! MRC files**. Type conversion is the caller's responsibility, with only a small
//! set of MRC-specific conveniences (e.g. `slices_f32` for the common i16→f32 case).

#![cfg_attr(feature = "f16", feature(f16))]

mod error;
mod header;
mod iter;
mod mode;
mod reader;
mod writer;

#[cfg(feature = "mmap")]
mod mmap_reader;

mod engine;

// Re-export core types
pub use engine::block::{SliceAccess, VolumeShape, VoxelBlock};
pub use engine::endian::FileEndian;

// Re-export codec trait for advanced users who need custom voxel types
pub use engine::codec::EndianCodec;

// Re-export MRC-specific format utilities
pub use engine::convert::{
    reinterpret_m0, unpack_u4_bytes_to_f32, unpack_u4_bytes_to_u16, unpack_u4_to_f32,
    unpack_u4_to_i8, unpack_u4_to_u16,
};

pub use error::{Error, HeaderValidationError};
pub use header::{ExtHeader, ExtHeaderMut, Header, HeaderBuilder};
pub use mode::{
    ComplexToRealStrategy, Float32Complex, Int16Complex, M0Interpretation, Mode, Packed4Bit, Voxel,
};
pub use reader::{Reader, SliceIterF32};
pub use iter::{BlockIter, SliceIter, SlabIter};
pub use writer::{Writer, WriterBuilder};

#[cfg(feature = "mmap")]
pub use writer::{MmapWriter, MmapWriterBuilder};

#[cfg(feature = "mmap")]
pub use mmap_reader::{MmapReader, MmapSliceIterF32, MmapBlockIter, MmapSliceIter, MmapSlabIter};

/// Open an MRC file for reading.
pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Reader, Error> {
    Reader::open(path)
}

/// Create a new MRC file for writing.
pub fn create<P: AsRef<std::path::Path>>(path: P) -> WriterBuilder {
    WriterBuilder::new(path)
}
