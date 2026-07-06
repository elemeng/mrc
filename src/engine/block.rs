//! Volume geometry and voxel block types.
//!
//! [`VolumeShape`] describes the dimensions of an MRC volume, and
//! [`VoxelBlock`] is the universal container for a contiguous chunk of
//! voxel data with a known 3D offset and shape.

use std::vec::Vec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Volume geometry in voxels.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VolumeShape {
    /// Number of columns — the fastest-changing (X) axis.
    pub nx: usize,
    /// Number of rows — the medium (Y) axis.
    pub ny: usize,
    /// Number of sections — the slowest-changing (Z) axis.
    pub nz: usize,
}

impl VolumeShape {
    /// Create a new volume shape.
    #[must_use]
    pub const fn new(nx: usize, ny: usize, nz: usize) -> Self {
        Self { nx, ny, nz }
    }

    /// Create a volume shape from an MRC header.
    ///
    /// Maps from the header's `nx`, `ny`, `nz` fields (stored as `i32`).
    /// Returns `Err(Error::bounds_err())` if any dimension is negative.
    pub fn from_header(header: &crate::Header) -> Result<Self, crate::Error> {
        Ok(Self {
            nx: usize::try_from(header.nx).map_err(|_| crate::Error::bounds_err())?,
            ny: usize::try_from(header.ny).map_err(|_| crate::Error::bounds_err())?,
            nz: usize::try_from(header.nz).map_err(|_| crate::Error::bounds_err())?,
        })
    }

    /// Total number of voxels, or `None` if the calculation overflows.
    pub fn total_voxels(&self) -> Option<usize> {
        self.nx.checked_mul(self.ny)?.checked_mul(self.nz)
    }

    /// Returns `true` if any dimension is zero.
    pub const fn is_empty(&self) -> bool {
        self.nx == 0 || self.ny == 0 || self.nz == 0
    }

    /// Check if a block with given offset and shape fits within this volume.
    /// Returns true if the block is completely within bounds.
    pub fn contains_block(&self, offset: [usize; 3], shape: [usize; 3]) -> bool {
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;
        ox.checked_add(sx).is_some_and(|x| x <= self.nx)
            && oy.checked_add(sy).is_some_and(|y| y <= self.ny)
            && oz.checked_add(sz).is_some_and(|z| z <= self.nz)
    }

    /// Compute the linear voxel index for a given 3D offset.
    /// Returns `None` if the calculation overflows `usize`.
    pub fn checked_linear_index(&self, offset: [usize; 3]) -> Option<usize> {
        let [ox, oy, oz] = offset;
        ox.checked_add(
            oy.checked_mul(self.nx)?
                .checked_add(oz.checked_mul(self.nx)?.checked_mul(self.ny)?)?,
        )
    }
}

/// A contiguous chunk of voxel data with a 3D offset and shape.
///
/// Created by [`VoxelBlock::new`] or returned by reader methods such as
/// [`Reader::slices`](crate::ReaderMethods::slices) and [`Reader::subregion`](crate::ReaderMethods::subregion).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct VoxelBlock<T> {
    /// Corner of the block within the volume, in voxels `[x, y, z]`.
    pub offset: [usize; 3],
    /// Extent of the block along each axis `[sx, sy, sz]`.
    pub shape: [usize; 3],
    /// Contiguous voxel values in C-order (X fastest, Z slowest).
    pub data: Vec<T>,
}

impl<T> VoxelBlock<T> {
    /// Create a new voxel block, returning an error if `data.len()` does not match `shape`.
    ///
    /// # Errors
    /// Returns [`crate::Error::BoundsError`] if `shape` dimensions overflow `usize`.
    /// Returns [`crate::Error::BlockShapeMismatch`] if `data.len()` does not match `shape`.
    pub fn new(offset: [usize; 3], shape: [usize; 3], data: Vec<T>) -> Result<Self, crate::Error> {
        Self::try_new(offset, shape, data)
    }

    /// Create a new VoxelBlock, returning an error if the data length does not
    /// match the shape.
    ///
    /// # Errors
    /// Returns [`crate::Error::BoundsError`] if `shape` dimensions overflow `usize`.
    /// Returns [`crate::Error::BlockShapeMismatch`] if `data.len()` does not match `shape`.
    pub fn try_new(
        offset: [usize; 3],
        shape: [usize; 3],
        data: Vec<T>,
    ) -> Result<Self, crate::Error> {
        let expected = shape[0]
            .checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or(crate::Error::bounds_err())?;
        if data.len() != expected {
            return Err(crate::Error::BlockShapeMismatch {
                expected,
                actual: data.len(),
            });
        }
        Ok(Self {
            offset,
            shape,
            data,
        })
    }

    /// Number of voxel values in this block.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if this block contains no voxels.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns `true` if this block covers the entire volume starting at the origin.
    pub fn is_full_volume(&self, volume_shape: &VolumeShape) -> bool {
        self.offset == [0, 0, 0]
            && self.shape == [volume_shape.nx, volume_shape.ny, volume_shape.nz]
    }
}
