//! Unified voxel access traits
//!
//! This module provides a consolidated trait hierarchy for type-safe voxel access:
//! - `VoxelAccess`: Basic read access (linear indexing)
//! - `VoxelAccessMut`: Mutable access
//! - `VolumeAccess`: Full 3D volume access with dimensions, strides, and iteration
//! - `VolumeAccessMut`: Mutable 3D volume access

use crate::core::{Error, Mode};
use crate::header::Header;
use crate::voxel::{Encoding, FileEndian, Voxel};
use alloc::vec::Vec;

/// Trait for read-only voxel access
///
/// This trait provides a unified interface for reading voxel data from
/// any Volume with runtime type checking.
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
/// implemented by mutable Volume instances.
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

/// Core Volume trait - zero-cost abstraction over storage backends
///
/// This trait provides a unified interface for volume data access.
/// Implementations are zero-cost - all methods inline down to direct memory access.
pub trait VolumeAccess: VoxelAccess {
    /// The voxel type stored in this volume
    type Voxel: Voxel + Encoding;

    /// Get the header
    fn header(&self) -> &Header;

    /// Get the dimensions (nx, ny, nz)
    fn dimensions(&self) -> (usize, usize, usize);

    /// Get the axis map
    #[inline]
    fn axis_map(&self) -> &crate::core::AxisMap {
        &self.header().axis_map
    }

    /// Get file endianness
    #[inline]
    fn endian(&self) -> FileEndian {
        self.header().file_endian
    }

    /// Get strides for index calculation
    fn strides(&self) -> (usize, usize, usize);

    /// Get a voxel without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn get_unchecked(&self, index: usize) -> Self::Voxel;

    /// Get a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds
    #[inline]
    fn get_at(&self, x: usize, y: usize, z: usize) -> Self::Voxel {
        let strides = self.strides();
        let index = x * strides.0 + y * strides.1 + z * strides.2;
        // SAFETY: Caller assumes proper bounds
        unsafe { self.get_unchecked(index) }
    }

    /// Get a voxel at 3D coordinates, returning None if out of bounds
    #[inline]
    fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<Self::Voxel> {
        let (nx, ny, nz) = self.dimensions();
        if x >= nx || y >= ny || z >= nz {
            return None;
        }
        Some(self.get_at(x, y, z))
    }

    /// Iterate over all voxels in storage order
    #[inline]
    fn iter(&self) -> VolumeIter<'_, Self>
    where
        Self: Sized,
    {
        VolumeIter {
            volume: self,
            index: 0,
        }
    }

    /// Map voxels to a new vector
    #[inline]
    fn map<F, U>(&self, mut f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U,
        Self: Sized,
    {
        (0..self.len())
            .map(|i| unsafe { f(self.get_unchecked(i)) })
            .collect()
    }

    /// Compute statistics for this volume
    fn compute_stats(&self) -> crate::stats::Statistics
    where
        Self::Voxel: Into<f64>,
        Self: Sized,
    {
        crate::stats::compute_stats(self.iter().map(|v| v.into()))
    }
}

/// Mutable volume trait
pub trait VolumeAccessMut: VolumeAccess + VoxelAccessMut {
    /// Set a voxel without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn set_unchecked(&mut self, index: usize, value: Self::Voxel);

    /// Set a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds
    #[inline]
    fn set_at(&mut self, x: usize, y: usize, z: usize, value: Self::Voxel) {
        let strides = self.strides();
        let index = x * strides.0 + y * strides.1 + z * strides.2;
        unsafe {
            self.set_unchecked(index, value);
        }
    }

    /// Set a voxel at 3D coordinates, returning error if out of bounds
    #[inline]
    fn set_at_checked(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        value: Self::Voxel,
    ) -> Result<(), Error> {
        let (nx, ny, nz) = self.dimensions();
        if x >= nx || y >= ny || z >= nz {
            return Err(Error::IndexOutOfBounds {
                index: x + y * nx + z * nx * ny,
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }
}

/// Iterator over volume voxels
pub struct VolumeIter<'a, V: VolumeAccess> {
    volume: &'a V,
    index: usize,
}

impl<V: VolumeAccess> Iterator for VolumeIter<'_, V> {
    type Item = V::Voxel;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.volume.len() {
            let item = unsafe { self.volume.get_unchecked(self.index) };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.volume.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<V: VolumeAccess> ExactSizeIterator for VolumeIter<'_, V> {}