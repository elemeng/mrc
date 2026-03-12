//! High-level Volume trait for zero-cost abstractions
//!
//! This module provides a unified trait-based API for volumes that works
//! across different storage backends with zero runtime overhead.

extern crate alloc;
use alloc::vec::Vec;

use super::{VoxelAccess, VoxelAccessMut};
use crate::core::{AxisMap, Error};
use crate::header::Header;
use crate::voxel::{Encoding, FileEndian, Voxel};

/// Core Volume trait - zero-cost abstraction over storage backends
///
/// This trait provides a unified interface for volume data access.
/// Implementations are zero-cost - all methods inline down to direct memory access.
pub trait Volume: VoxelAccess {
    /// The voxel type stored in this volume
    type Voxel: Voxel + Encoding;

    /// Get the header
    fn header(&self) -> &Header;

    /// Get the shape (nx, ny, nz)
    fn shape(&self) -> (usize, usize, usize);

    /// Get the axis map
    #[inline]
    fn axis_map(&self) -> &AxisMap {
        &self.header().axis_map
    }

    /// Get file endianness
    #[inline]
    fn endian(&self) -> FileEndian {
        self.header().file_endian
    }

    /// Get a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds
    #[inline]
    fn get_at(&self, x: usize, y: usize, z: usize) -> Self::Voxel {
        let strides = self.strides();
        let index = x * strides.0 + y * strides.1 + z * strides.2;
        // SAFETY: We assume the implementation has proper bounds checking
        // or the caller has verified coordinates
        unsafe { self.get_unchecked(index) }
    }

    /// Get a voxel at 3D coordinates, returning None if out of bounds
    #[inline]
    fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<Self::Voxel> {
        let (nx, ny, nz) = self.shape();
        if x >= nx || y >= ny || z >= nz {
            return None;
        }
        Some(self.get_at(x, y, z))
    }

    /// Get strides for index calculation
    fn strides(&self) -> (usize, usize, usize);

    /// Get a voxel without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn get_unchecked(&self, index: usize) -> Self::Voxel;

    /// Compute statistics for this volume
    fn compute_stats(&self) -> VolumeStats
    where
        Self::Voxel: Into<f64>;
}

/// Mutable volume trait
pub trait VolumeMut: Volume + VoxelAccessMut {
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
        let (nx, ny, nz) = self.shape();
        if x >= nx || y >= ny || z >= nz {
            return Err(Error::IndexOutOfBounds {
                index: x + y * nx + z * nx * ny,
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }

    /// Set a voxel without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn set_unchecked(&mut self, index: usize, value: Self::Voxel);
}

// Re-export Statistics from stats module for consistency
pub use crate::stats::Statistics as VolumeStats;

/// Extension trait for volume operations
pub trait VolumeExt: Volume {
    /// Iterate over all voxels
    fn iter(&self) -> VolumeIter<'_, Self>;

    /// Map voxels to a new volume
    fn map<F, U>(&self, f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U;
}

impl<V: Volume> VolumeExt for V {
    #[inline]
    fn iter(&self) -> VolumeIter<'_, Self> {
        VolumeIter {
            volume: self,
            index: 0,
        }
    }

    #[inline]
    fn map<F, U>(&self, mut f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U,
    {
        (0..self.len())
            .map(|i| unsafe { f(self.get_unchecked(i)) })
            .collect()
    }
}

/// Iterator over volume voxels
pub struct VolumeIter<'a, V: Volume + ?Sized> {
    volume: &'a V,
    index: usize,
}

impl<V: Volume> Iterator for VolumeIter<'_, V> {
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

impl<V: Volume> ExactSizeIterator for VolumeIter<'_, V> {}
