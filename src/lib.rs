//! # MRC File Format Library
//!
//! A zero-copy, zero-allocation MRC-2014 file format reader/writer for Rust.
//!
//! This crate provides high-performance, memory-efficient access to MRC
//! (Medical Research Council) files used in cryo-electron microscopy and
//! structural biology.
//!
//! ## Features
//!
//! - **Zero-copy access**: Direct slice views into data without allocation
//! - **no_std compatible**: Works in embedded and WebAssembly environments
//! - **Type-safe**: Compile-time mode checking with generics
//! - **Memory safe**: Lifetime-based borrowing prevents use-after-free
//!
//! ## Quick Start
//!
//! ```ignore
//! use mrc::{MrcReader, Mode};
//!
//! // Read an MRC file
//! let reader = MrcReader::open("data.mrc")?;
//! let header = reader.header();
//! let volume = reader.read_volume::<f32>()?;
//!
//! // Access voxel data
//! let value = volume.get(x, y, z);
//! ```

#![no_std]

#[cfg(feature = "f16")]
extern crate half;

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// Core types
pub mod axis;
pub mod encoding;
pub mod endian;
pub mod error;
pub mod mode;
pub mod voxel;

// Header module
pub mod header;

// Re-exports
pub use axis::AxisMap;
pub use encoding::Encoding;
pub use endian::FileEndian;
pub use error::Error;
pub use header::{Header, RawHeader};
pub use mode::{InvalidMode, Mode};
pub use voxel::{ComplexVoxel, RealVoxel, ScalarVoxel, Voxel, Int16Complex, Float32Complex};

// Feature-gated modules
#[cfg(feature = "std")]
pub mod io;

#[cfg(feature = "std")]
pub mod storage;

#[cfg(feature = "std")]
pub mod volume;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mode_from_i32() {
        assert_eq!(Mode::from_i32(0), Some(Mode::Int8));
        assert_eq!(Mode::from_i32(1), Some(Mode::Int16));
        assert_eq!(Mode::from_i32(2), Some(Mode::Float32));
        assert_eq!(Mode::from_i32(99), None);
    }
    
    #[test]
    fn test_axis_map() {
        let map = AxisMap::default();
        assert!(map.is_standard());
        assert!(map.validate());
    }
    
    #[test]
    fn test_file_endian() {
        let native = FileEndian::native();
        assert!(native.is_native());
    }
    
    #[test]
    fn test_raw_header_new() {
        let header = RawHeader::new();
        assert!(header.has_valid_map());
    }
    
    #[test]
    fn test_header_default() {
        let header = Header::default();
        assert_eq!(header.dimensions(), (1, 1, 1));
    }
}
