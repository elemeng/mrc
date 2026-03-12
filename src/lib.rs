//! # MRC File Format Library
//!
//! This crate provides a safe, efficient, and endian-correct implementation for reading
//! and writing MRC (Medical Research Council) files, which are commonly used in
//! cryo-electron microscopy and structural biology.
//!
//! ## Memory Model
//!
//! This crate strictly separates the three components of an MRC file:
//!
//! ```text
//! File layout:  | 1024 bytes | NSYMBT bytes | data_size bytes |
//!               | Header     | ExtHeader    | VoxelData       |
//!
//! Memory model: | Header     | ExtHeader    | VoxelData       |
//!               | (decoded)  | (raw bytes)  | (raw bytes)     |
//!               | native-end| opaque       | file-endian     |
//! ```
//!
//! - **Header** (1024 bytes): Always decoded on load, always native-endian in memory
//! - **Extended header** (NSYMBT bytes): Opaque bytes, no endianness conversion
//! - **Voxel data** (data_size bytes): Raw bytes in file-endian, decoded lazily on access
//!
//! Endianness conversion occurs **only** when decoding or encoding typed numeric values
//! through the `DecodeFromFile` and `EncodeToFile` traits. This ensures zero-copy mmap
//! views and prevents accidental endian corruption.
//!
//! ## Features
//!
//! - `std`: Standard library support for file I/O
//! - `mmap`: Memory-mapped file support for zero-copy access
//! - `f16`: Half-precision floating point support (via `half` crate)
//!
//! ## Safety
//!
//! All public operations are memory-safe. The crate's public API contains no unsafe
//! code for data access, and all endianness conversions are performed through safe,
//! type-checked APIs.
//!
//! ### Memory Mapping
//! When using the `mmap` feature, the underlying OS memory mapping is created using
//! `unsafe` code internally (as required by the `memmap2` crate). However, the public
//! API remains safe - the `MrcMmap` type encapsulates the mapped memory and ensures
//! valid access through Rust's borrowing rules.
//!
//! ## Endianness Policy
//!
//! This crate enforces a simple and safe endianness model:
//!
//! - All newly created MRC files are written in little-endian format.
//! - Existing MRC files are read and modified using their declared file endianness.
//! - Endianness is handled internally during numeric decode/encode.
//! - Users never need to reason about byte order.
//!
//! This guarantees compatibility with the MRC2014 ecosystem while supporting
//! cross-platform reading, writing, memory-mapped access, and streaming updates.

#![no_std]

#[cfg(feature = "f16")]
extern crate half;

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// Internal modules
mod data;
mod endian;
mod error;
mod header;
mod mode;
mod traits;
mod types;
mod view;

#[cfg(test)]
#[path = "../test/tests.rs"]
mod tests;

// Public re-exports

// Core types
pub use data::{DataBlock, DataBlockMut, ExtHeader, ExtHeaderMut};
pub use endian::FileEndian;
pub use error::Error;
pub use header::Header;
pub use mode::Mode;
pub use traits::{DecodeFromFile, EncodeToFile, VoxelType};
pub use types::{Float32Complex, Int16Complex, Packed4Bit};
pub use view::{MrcView, MrcViewMut};

// Optional file features
#[cfg(feature = "file")]
mod mrcfile;

#[cfg(test)]
#[cfg(feature = "file")]
#[path = "../test/mrcfile_test.rs"]
mod mrcfile_test;

#[cfg(feature = "mmap")]
pub use mrcfile::{open_mmap, MrcMmap};

#[cfg(feature = "file")]
pub use mrcfile::{MrcFile, MrcSource};