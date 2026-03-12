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

// Core types
pub mod access;
pub mod axis;
pub mod data_block;
pub mod encoding;
pub mod endian;
pub mod error;
pub mod mode;
pub mod stats;
pub mod voxel;

// Header module
pub mod header;

// Extended header support
pub mod extended;

// Re-exports
pub use access::{VoxelAccess, VoxelAccessMut};
pub use axis::AxisMap;
pub use data_block::{DataBlock, DataBlockMut};
pub use encoding::Encoding;
pub use endian::FileEndian;
pub use error::Error;
pub use header::{Header, HeaderBuilder, RawHeader};
pub use mode::{InvalidMode, Mode};
pub use stats::{compute_stats, compute_stats_slice, RunningStats, Statistics as Stats};
pub use voxel::{ComplexVoxel, IntegerVoxel, RealVoxel, ScalarVoxel, Voxel, Int16Complex, Float32Complex, Packed4Bit};

// Feature-gated modules
#[cfg(feature = "std")]
pub mod io;

#[cfg(feature = "std")]
pub mod volume;

#[cfg(feature = "std")]
pub mod volume_trait;

#[cfg(feature = "std")]
pub mod dynamic;

#[cfg(feature = "std")]
pub use dynamic::VolumeData;

#[cfg(feature = "std")]
pub use extended::ExtendedHeader;

#[cfg(feature = "std")]
pub use io::{MrcReader, MrcWriter};

#[cfg(feature = "std")]
pub use volume::{Statistics, Volume};

#[cfg(feature = "std")]
pub use volume_trait::{Volume as VolumeTrait, VolumeMut, VolumeRef, VolumeMutRef, VolumeStats, VolumeExt};

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
        assert_eq!(extended::ExtType::from_bytes(b"CCP4"), extended::ExtType::Ccp4);
        assert_eq!(extended::ExtType::from_bytes(b"SERI"), extended::ExtType::Seri);
    }
}