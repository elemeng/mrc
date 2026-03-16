//! MRC file format library for cryo-EM and tomography
//!
//! Provides iterator-centric reading and voxel-block writing with SIMD acceleration.

#![no_std]
#![cfg_attr(feature = "f16", feature(f16))]

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod header;
mod mode;
mod block;
mod decode;
mod encode;
mod iter;
mod reader;
mod writer;
mod error;
mod endian;

pub use header::{Header, ExtHeader, ExtHeaderMut};
pub use mode::{Mode, Int16Complex, Float32Complex, Packed4Bit};
pub use block::{VolumeShape, VoxelBlock};
pub use reader::Reader;
pub use writer::{Writer, WriterBuilder};
pub use error::Error;
pub use endian::FileEndian;
pub use decode::Decode;
pub use encode::Encode;

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