//! Unified volume access traits
//!
//! Simplified trait hierarchy:
//! - `VolumeAccess`: Statically typed 3D volume access with spatial coordinates
//! - `VolumeAccessMut`: Mutable 3D volume access

use crate::core::{Error, Mode};
use crate::header::Header;
use crate::voxel::{Encoding, Voxel};
use alloc::vec::Vec;

/// Statically typed 3D volume access (compile-time mode checking)
///
/// This trait provides spatial access to volume data with clear (x, y, z) coordinates.
/// Linear indexing is not provided - use iterators or coordinates instead.
pub trait VolumeAccess {
    /// The voxel type stored in this volume
    type Voxel: Voxel + Encoding;

    /// Get the header
    fn header(&self) -> &Header;

    /// Get the mode
    #[inline]
    fn mode(&self) -> Mode {
        self.header().mode()
    }

    /// Get dimensions (nx, ny, nz)
    fn dimensions(&self) -> (usize, usize, usize);

    /// Get the axis map
    #[inline]
    fn axis_map(&self) -> &crate::core::AxisMap {
        self.header().axis_map()
    }

    /// Get strides for index calculation
    fn strides(&self) -> (usize, usize, usize);

    /// Total number of voxels
    fn len(&self) -> usize;

    /// Check if empty
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if coordinates are in bounds
    #[inline]
    fn in_bounds(&self, x: usize, y: usize, z: usize) -> bool {
        let (nx, ny, nz) = self.dimensions();
        x < nx && y < ny && z < nz
    }

    /// Compute linear index from 3D coordinates
    #[inline]
    fn linear_index(&self, x: usize, y: usize, z: usize) -> usize {
        let (sx, sy, sz) = self.strides();
        x * sx + y * sy + z * sz
    }

    /// Get a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds
    fn get_at(&self, x: usize, y: usize, z: usize) -> Self::Voxel;

    /// Get a voxel at 3D coordinates, returning None if out of bounds
    fn get_at_opt(&self, x: usize, y: usize, z: usize) -> Option<Self::Voxel> {
        self.in_bounds(x, y, z).then(|| self.get_at(x, y, z))
    }

    /// Get a voxel at 3D coordinates with full error context
    fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Result<Self::Voxel, Error> {
        if !self.in_bounds(x, y, z) {
            let (nx, ny, nz) = self.dimensions();
            return Err(Error::IndexOutOfBounds {
                index: self.linear_index(x, y, z),
                length: nx * ny * nz,
            });
        }
        Ok(self.get_at(x, y, z))
    }

    /// Get voxel without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn get_unchecked(&self, index: usize) -> Self::Voxel;

    /// Iterate over all voxels in storage order
    fn iter(&self) -> VolumeIter<'_, Self>
    where
        Self: Sized,
    {
        VolumeIter::new(self)
    }

    /// Iterate over coordinates in logical order (X varies fastest)
    fn iter_coords(&self) -> impl Iterator<Item = (usize, usize, usize)> {
        let (nx, ny, nz) = self.dimensions();
        (0..nz).flat_map(move |z| (0..ny).flat_map(move |y| (0..nx).map(move |x| (x, y, z))))
    }

    /// Iterate over (coordinate, voxel) pairs
    fn iter_with_coords(&self) -> impl Iterator<Item = ((usize, usize, usize), Self::Voxel)>
    where
        Self: Sized,
    {
        let (nx, ny, nz) = self.dimensions();
        (0..nz).flat_map(move |z| {
            (0..ny).flat_map(move |y| {
                (0..nx).map(move |x| {
                    let voxel = self.get_at(x, y, z);
                    ((x, y, z), voxel)
                })
            })
        })
    }

    /// Map voxels to a new vector
    fn map<F, U>(&self, f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U,
        Self: Sized,
    {
        self.iter().map(f).collect()
    }

    /// Fold over all voxels
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

/// Mutable statically typed 3D volume access
pub trait VolumeAccessMut: VolumeAccess {
    /// Set a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds
    fn set_at(&mut self, x: usize, y: usize, z: usize, value: Self::Voxel);

    /// Set a voxel at 3D coordinates, returning None if out of bounds
    fn set_at_opt(&mut self, x: usize, y: usize, z: usize, value: Self::Voxel) -> bool {
        if self.in_bounds(x, y, z) {
            self.set_at(x, y, z, value);
            true
        } else {
            false
        }
    }

    /// Set a voxel at 3D coordinates with full error context
    fn set_at_checked(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        value: Self::Voxel,
    ) -> Result<(), Error> {
        if !self.in_bounds(x, y, z) {
            let (nx, ny, nz) = self.dimensions();
            return Err(Error::IndexOutOfBounds {
                index: self.linear_index(x, y, z),
                length: nx * ny * nz,
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }

    /// Fill all voxels with the same value
    fn fill(&mut self, value: Self::Voxel)
    where
        Self::Voxel: Copy,
    {
        let (nx, ny, nz) = self.dimensions();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    self.set_at(x, y, z, value);
                }
            }
        }
    }

    /// Apply a transformation to each voxel in-place
    fn transform<F>(&mut self, f: F)
    where
        F: FnMut(Self::Voxel) -> Self::Voxel,
    {
        let (nx, ny, nz) = self.dimensions();
        let mut f = f;
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let old = self.get_at(x, y, z);
                    self.set_at(x, y, z, f(old));
                }
            }
        }
    }
}

/// Iterator over volume voxels in storage order
pub struct VolumeIter<'a, V: VolumeAccess + ?Sized> {
    volume: &'a V,
    index: usize,
    len: usize,
}

impl<'a, V: VolumeAccess + ?Sized> VolumeIter<'a, V> {
    fn new(volume: &'a V) -> Self {
        Self {
            index: 0,
            len: volume.len(),
            volume,
        }
    }
}

impl<V: VolumeAccess> Iterator for VolumeIter<'_, V> {
    type Item = V::Voxel;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            let idx = self.index;
            self.index += 1;
            // SAFETY: We just checked index < len
            unsafe { Some(self.volume.get_unchecked(idx)) }
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<V: VolumeAccess> ExactSizeIterator for VolumeIter<'_, V> {}
