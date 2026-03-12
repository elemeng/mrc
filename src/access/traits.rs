//! Unified voxel access traits
//!
//! This module provides traits for type-safe voxel access, implemented by
//! both `DataBlock` and `Volume` to provide a consistent API.

use crate::core::{Error, Mode};
use crate::voxel::{Encoding, Voxel};

/// Trait for read-only voxel access
///
/// This trait provides a unified interface for reading voxel data from
/// any source (DataBlock, Volume, etc.) with runtime type checking.
pub trait VoxelAccess {
    /// Get the data mode
    fn mode(&self) -> Mode;

    /// Get the number of voxels
    fn len(&self) -> usize;

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a voxel value of type T at the given index
    ///
    /// # Type Parameters
    /// * `T` - The voxel type to retrieve (must implement Voxel + Encoding)
    ///
    /// # Errors
    /// Returns `Error::TypeMismatch` if the requested type doesn't match the data mode
    /// Returns `Error::IndexOutOfBounds` if index is out of range
    fn get<T: Voxel + Encoding>(&self, index: usize) -> Result<T, Error>;
}

/// Trait for mutable voxel access
///
/// This trait provides a unified interface for modifying voxel data,
/// implemented by both `DataBlockMut` and mutable `Volume` instances.
pub trait VoxelAccessMut: VoxelAccess {
    /// Set a voxel value of type T at the given index
    ///
    /// # Type Parameters
    /// * `T` - The voxel type to set (must implement Voxel + Encoding)
    ///
    /// # Errors
    /// Returns `Error::TypeMismatch` if the value type doesn't match the data mode
    /// Returns `Error::IndexOutOfBounds` if index is out of range
    fn set<T: Voxel + Encoding>(&mut self, index: usize, value: T) -> Result<(), Error>;
}
