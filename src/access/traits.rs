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

    /// Try to get a voxel at index, returning None if out of bounds
    fn get_opt<T: Voxel + Encoding>(&self, index: usize) -> Option<T> {
        (index < self.len()).then(|| self.get(index).ok()).flatten()
    }

    /// Collect all voxels of type T into a vector
    fn collect_vec<T: Voxel + Encoding>(&self) -> Result<Vec<T>, Error> {
        (0..self.len()).map(|i| self.get(i)).collect()
    }
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

    /// Try to set a voxel at index, returning false if out of bounds
    fn set_opt<T: Voxel + Encoding>(&mut self, index: usize, value: T) -> bool {
        index < self.len() && self.set(index, value).is_ok()
    }

    /// Fill all voxels with the same value
    fn fill<T: Voxel + Encoding + Copy>(&mut self, value: T) -> Result<(), Error> {
        (0..self.len()).try_for_each(|i| self.set(i, value))
    }
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

    /// Check if coordinates are in bounds
    #[inline]
    fn in_bounds(&self, x: usize, y: usize, z: usize) -> bool {
        let (nx, ny, nz) = self.dimensions();
        x < nx && y < ny && z < nz
    }

    /// Compute linear index from 3D coordinates
    #[inline]
    fn linear_index(&self, x: usize, y: usize, z: usize) -> usize {
        let strides = self.strides();
        x * strides.0 + y * strides.1 + z * strides.2
    }

    /// Get a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds
    #[inline]
    fn get_at(&self, x: usize, y: usize, z: usize) -> Self::Voxel {
        let index = self.linear_index(x, y, z);
        // SAFETY: Caller assumes proper bounds
        unsafe { self.get_unchecked(index) }
    }

    /// Get a voxel at 3D coordinates, returning None if out of bounds
    #[inline]
    fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<Self::Voxel> {
        self.in_bounds(x, y, z).then(|| self.get_at(x, y, z))
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

    /// Iterate over coordinates in logical order
    #[inline]
    fn iter_coords(&self) -> impl Iterator<Item = (usize, usize, usize)> {
        let (nx, ny, nz) = self.dimensions();
        (0..nz).flat_map(move |z| {
            (0..ny).flat_map(move |y| (0..nx).map(move |x| (x, y, z)))
        })
    }

    /// Iterate over (coordinate, voxel) pairs
    #[inline]
    fn iter_with_coords(&self) -> impl Iterator<Item = ((usize, usize, usize), Self::Voxel)>
    where
        Self: Sized,
    {
        self.iter_coords()
            .map(move |coords| (coords, self.get_at(coords.0, coords.1, coords.2)))
    }

    /// Map voxels to a new vector
    #[inline]
    fn map<F, U>(&self, f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U,
        Self: Sized,
    {
        self.iter().map(f).collect()
    }

    /// Filter voxels and collect matching ones
    #[inline]
    fn filter<F>(&self, predicate: F) -> Vec<Self::Voxel>
    where
        F: FnMut(&Self::Voxel) -> bool,
        Self: Sized,
    {
        self.iter().filter(predicate).collect()
    }

    /// Fold over all voxels
    #[inline]
    fn fold<B, F>(&self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Voxel) -> B,
        Self: Sized,
    {
        self.iter().fold(init, f)
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
        let index = self.linear_index(x, y, z);
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
        if !self.in_bounds(x, y, z) {
            return Err(Error::IndexOutOfBounds {
                index: self.linear_index(x, y, z),
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }

    /// Set a voxel at 3D coordinates, returning false if out of bounds
    #[inline]
    fn set_at_opt(&mut self, x: usize, y: usize, z: usize, value: Self::Voxel) -> bool {
        self.in_bounds(x, y, z).then(|| self.set_at(x, y, z, value)).is_some()
    }

    /// Apply a function to each voxel in-place
    fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Self::Voxel) -> Self::Voxel,
    {
        for i in 0..self.len() {
            unsafe {
                let old = self.get_unchecked(i);
                self.set_unchecked(i, f(old));
            }
        }
    }

    /// Fill all voxels with the same value
    fn fill_volume(&mut self, value: Self::Voxel)
    where
        Self::Voxel: Copy,
    {
        let len = self.len();
        for i in 0..len {
            // SAFETY: i is in bounds since we iterate to len
            unsafe {
                self.set_unchecked(i, value);
            }
        }
    }

    /// Apply a transformation to each voxel
    fn transform<F>(&mut self, f: F)
    where
        F: Fn(Self::Voxel) -> Self::Voxel,
        Self::Voxel: Copy,
    {
        let len = self.len();
        for i in 0..len {
            // SAFETY: i is in bounds since we iterate to len
            unsafe {
                let old = self.get_unchecked(i);
                self.set_unchecked(i, f(old));
            }
        }
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