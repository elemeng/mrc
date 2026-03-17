//! MRC file format library for cryo-EM and tomography
//!
//! Provides iterator-centric reading and voxel-block writing with SIMD acceleration.
//!
//! # Architecture
//!
//! This crate implements a unified encoding/decoding pipeline:
//! ```text
//! Raw Bytes → Endian Normalization → Typed Values → Type Conversion
//! ```
//!
//! With zero-copy fast paths whenever possible.

#![no_std]
#![cfg_attr(feature = "f16", feature(f16))]

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod error;
mod header;
mod iter;
mod mode;
mod reader;
mod writer;

mod engine;

// Re-export core types
pub use engine::block::{VolumeShape, VoxelBlock};
pub use engine::endian::FileEndian;

// Re-export codec trait for advanced users who need custom voxel types
pub use engine::codec::EndianCodec;

// Re-export conversion trait and pipeline types for type conversion
pub use engine::convert::Convert;
pub use engine::pipeline::{ConversionPath, get_conversion_path, is_zero_copy};

// Re-export SliceAccess trait for writers
pub use engine::block::SliceAccess;

pub use error::Error;
pub use header::{ExtHeader, ExtHeaderMut, Header};
pub use mode::{Float32Complex, Int16Complex, Mode, Packed4Bit, Voxel};
pub use reader::Reader;
pub use writer::{Writer, WriterBuilder};

// Re-export conversion-enabled iterators
pub use iter::{SliceIterConverted, SlabIterConverted};

#[cfg(feature = "mmap")]
pub use writer::MmapWriter;

/// Open an MRC file for reading
#[cfg(feature = "std")]
pub fn open(path: &str) -> Result<Reader, Error> {
    Reader::open(path)
}

/// Create a new MRC file for writing
#[cfg(feature = "std")]
pub fn create(path: &str) -> WriterBuilder {
    WriterBuilder::new(path)
}
