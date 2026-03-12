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
//! ```no_run
//! use mrc::{MrcReader, Mode, VolumeData};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Read an MRC file with compile-time type checking
//!     let mut reader = MrcReader::open("data.mrc")?;
//!     let volume = reader.read_volume::<f32>()?;
//!
//!     // Access voxel data
//!     let value = volume.get_at(10, 20, 5);
//!
//!     // Or use dynamic dispatch for unknown modes
//!     let mut reader = MrcReader::open("data.mrc")?;
//!     let data = reader.read()?;
//!     if let Some(vol) = data.as_f32() {
//!         // handle f32
//!     }
//!     Ok(())
//! }
//! ```

#![no_std]

#[cfg(feature = "f16")]
extern crate half;

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// Core types (no_std compatible)
pub mod core;
pub mod voxel;

// Header module
pub mod header;

// Data access patterns (std)
#[cfg(feature = "std")]
pub mod access;

// Statistics (std)
#[cfg(feature = "std")]
pub mod stats;

// IO operations (std)
#[cfg(feature = "std")]
pub mod io;

// Core re-exports
pub use core::{AxisMap, Error, Mode, check_bounds};

// Voxel re-exports
pub use voxel::{
    ComplexF32, ComplexI16, ComplexVoxel, EndianConvert, FileEndian, Float32Complex, Int16Complex,
    IntegerVoxel, Packed4Bit, RealVoxel, ScalarVoxel, Voxel,
};

// Header re-exports
pub use header::{Header, HeaderBuilder, RawHeader};

// Feature-gated re-exports
#[cfg(feature = "std")]
pub use access::{VoxelAccess, VoxelAccessMut, Volume, VolumeAccess, VolumeAccessMut, VolumeData, VolumeIter};

/// 1D volume type alias (replaces DataBlock)
#[cfg(feature = "std")]
pub type Volume1D<T, S> = Volume<T, S, 1>;

#[cfg(feature = "std")]
pub use header::{ExtType, ExtendedHeader};

#[cfg(feature = "std")]
pub use io::{MrcReader, MrcSink, MrcSource, MrcWriter};

#[cfg(feature = "std")]
pub use stats::{RunningStats, Statistics, compute_stats};

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

    #[test]
    fn test_ext_type() {
        assert_eq!(header::ExtType::from_bytes(b"CCP4"), header::ExtType::Ccp4);
        assert_eq!(header::ExtType::from_bytes(b"SERI"), header::ExtType::Seri);
    }
}