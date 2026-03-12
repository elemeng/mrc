//! Voxel types and encoding for MRC data
//!
//! This module provides:
//! - `Voxel`: Base trait for all voxel types
//! - Concrete voxel types (ComplexI16, ComplexF32, Packed4Bit)
//! - Endianness handling

pub mod codex;
pub mod endian;
pub mod types;

// Re-export main types from types
pub use types::{
    ComplexF32, ComplexI16, ComplexVoxel, Float32Complex, Int16Complex, IntegerVoxel, Packed4Bit,
    RealVoxel, ScalarVoxel, Voxel,
};

// Re-export from endian
pub use endian::{EndianConvert, FileEndian};

// Re-export from encoding
pub use codex::Encoding;

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
