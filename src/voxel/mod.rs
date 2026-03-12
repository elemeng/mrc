//! Voxel types and encoding for MRC data
//!
//! This module provides:
//! - `Voxel`: Base trait for all voxel types
//! - Concrete voxel types (ComplexI16, ComplexF32, Packed4Bit)
//! - Endianness handling

pub mod types;
pub mod endian;
pub mod encoding;

// Re-export main types from types
pub use types::{
    Voxel, ScalarVoxel, RealVoxel, IntegerVoxel, ComplexVoxel,
    ComplexI16, ComplexF32, Packed4Bit, Int16Complex, Float32Complex,
};

// Re-export from endian
pub use endian::{FileEndian, EndianConvert};

// Re-export from encoding
pub use encoding::Encoding;

use crate::core::{Error, Mode};

/// Validate that voxel type T matches the expected mode
/// 
/// Returns Ok(()) if the mode matches, Err(Error::TypeMismatch) otherwise
#[inline]
pub fn validate_mode<T: Voxel>(expected: Mode) -> Result<(), Error> {
    if T::MODE != expected {
        return Err(Error::TypeMismatch);
    }
    Ok(())
}