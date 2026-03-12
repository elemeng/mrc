//! Voxel types and encoding for MRC data
//!
//! This module provides:
//! - `Voxel`: Base trait for all voxel types
//! - Concrete voxel types (ComplexI16, ComplexF32, Packed4Bit)
//! - Endianness handling (internal)

pub mod codex;
pub mod endian;
pub mod types;

pub use types::{
    ComplexF32, ComplexI16, ComplexVoxel, IntegerVoxel, Packed4Bit, RealVoxel, ScalarVoxel, Voxel,
};

// FileEndian and EndianConvert are internal
pub(crate) use endian::EndianConvert;
pub(crate) use endian::FileEndian;

// Re-export Encoding for trait bounds (users don't call methods directly)
pub use codex::Encoding;

use crate::core::{Error, Mode};

/// Validate that voxel type T matches the expected mode
#[inline]
pub(crate) fn validate_mode<T: Voxel>(expected: Mode) -> Result<(), Error> {
    if T::MODE != expected {
        return Err(Error::TypeMismatch);
    }
    Ok(())
}
