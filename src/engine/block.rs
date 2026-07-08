//! Volume geometry and voxel block types.
//!
//! [`VolumeShape`] describes the dimensions of an MRC volume, and
//! [`VoxelBlock`] is the universal container for a contiguous chunk of
//! voxel data with a known 3D offset and shape.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Volume geometry in voxels.
///
/// # Examples
///
/// ```rust
/// use mrc::VolumeShape;
/// let shape = VolumeShape::new(64, 64, 64);
/// assert_eq!(shape.total_voxels(), Some(262_144));
/// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VolumeShape;
    /// let shape = VolumeShape::new(10, 20, 30);
    /// assert_eq!(shape.nx, 10);
    /// assert_eq!(shape.ny, 20);
    /// assert_eq!(shape.nz, 30);
    /// ```
    #[must_use]
    pub const fn new(nx: usize, ny: usize, nz: usize) -> Self {
        Self { nx, ny, nz }
    }

    /// Create a volume shape from an MRC header.
    ///
    /// Maps from the header's `nx`, `ny`, `nz` fields (stored as `i32`).
    /// Returns `Err(Error::bounds_err())` if any dimension is negative.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VolumeShape;
    /// use mrc::Header;
    /// let mut header = Header::new();
    /// header.nx = 64;
    /// header.ny = 128;
    /// header.nz = 256;
    /// let shape = VolumeShape::from_header(&header).unwrap();
    /// assert_eq!(shape.total_voxels(), Some(64 * 128 * 256));
    /// ```
    pub fn from_header(header: &crate::Header) -> Result<Self, crate::Error> {
        Ok(Self {
            nx: usize::try_from(header.nx).map_err(|_| crate::Error::bounds_err())?,
            ny: usize::try_from(header.ny).map_err(|_| crate::Error::bounds_err())?,
            nz: usize::try_from(header.nz).map_err(|_| crate::Error::bounds_err())?,
        })
    }

    /// Total number of voxels, or `None` if the calculation overflows.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VolumeShape;
    /// let shape = VolumeShape::new(4, 5, 6);
    /// assert_eq!(shape.total_voxels(), Some(120));
    /// let empty = VolumeShape::new(4, 5, 0);
    /// assert_eq!(empty.total_voxels(), Some(0));
    /// ```
    pub fn total_voxels(&self) -> Option<usize> {
        self.nx.checked_mul(self.ny)?.checked_mul(self.nz)
    }

    /// Returns `true` if any dimension is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VolumeShape;
    /// let shape = VolumeShape::new(10, 10, 10);
    /// assert!(!shape.is_empty());
    /// let empty = VolumeShape::new(0, 10, 10);
    /// assert!(empty.is_empty());
    /// ```
    pub const fn is_empty(&self) -> bool {
        self.nx == 0 || self.ny == 0 || self.nz == 0
    }

    /// Check if a block with given offset and shape fits within this volume.
    /// Returns true if the block is completely within bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VolumeShape;
    /// let shape = VolumeShape::new(10, 10, 10);
    /// assert!(shape.contains_block([0, 0, 0], [5, 5, 5]));
    /// assert!(!shape.contains_block([5, 5, 5], [6, 5, 5]));
    /// ```
    pub fn contains_block(&self, offset: [usize; 3], shape: [usize; 3]) -> bool {
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;
        ox.checked_add(sx).is_some_and(|x| x <= self.nx)
            && oy.checked_add(sy).is_some_and(|y| y <= self.ny)
            && oz.checked_add(sz).is_some_and(|z| z <= self.nz)
    }

    /// Compute the linear voxel index for a given 3D offset.
    /// Returns `None` if the calculation overflows `usize`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VolumeShape;
    /// let shape = VolumeShape::new(4, 3, 2);
    /// assert_eq!(shape.checked_linear_index([0, 0, 0]), Some(0));
    /// assert_eq!(shape.checked_linear_index([3, 2, 1]), Some(23));
    /// assert_eq!(shape.checked_linear_index([0, 0, usize::MAX]), None);
    /// ```
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
/// [`crate::Reader::slices`] and [`crate::Reader::subregion`].
///
/// # Examples
///
/// ```rust
/// use mrc::{VoxelBlock, VolumeShape};
/// let block = VoxelBlock::new([0, 0, 0], [2, 2, 2], vec![0u8; 8]).unwrap();
/// assert_eq!(block.len(), 8);
/// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VoxelBlock;
    /// let block = VoxelBlock::new([1, 2, 3], [4, 4, 4], vec![0i16; 64]).unwrap();
    /// assert_eq!(block.offset, [1, 2, 3]);
    /// ```
    pub fn new(offset: [usize; 3], shape: [usize; 3], data: Vec<T>) -> Result<Self, crate::Error> {
        Self::try_new(offset, shape, data)
    }

    /// Create a new VoxelBlock, returning an error if the data length does not
    /// match the shape.
    ///
    /// # Errors
    /// Returns [`crate::Error::BoundsError`] if `shape` dimensions overflow `usize`.
    /// Returns [`crate::Error::BlockShapeMismatch`] if `data.len()` does not match `shape`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VoxelBlock;
    /// let block = VoxelBlock::try_new([0, 0, 0], [2, 2, 2], vec![1.0f32; 8]).unwrap();
    /// assert_eq!(block.len(), 8);
    /// assert!(VoxelBlock::<f32>::try_new([0, 0, 0], [2, 2, 2], vec![1.0f32; 7]).is_err());
    /// ```
    pub fn try_new(
        offset: [usize; 3],
        shape: [usize; 3],
        data: Vec<T>,
    ) -> Result<Self, crate::Error> {
        let expected = shape[0]
            .checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or_else(crate::Error::bounds_err)?;
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VoxelBlock;
    /// let block = VoxelBlock::new([0, 0, 0], [3, 3, 3], vec![0u16; 27]).unwrap();
    /// assert_eq!(block.len(), 27);
    /// ```
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if this block contains no voxels.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::VoxelBlock;
    /// let block = VoxelBlock::new([0, 0, 0], [1, 1, 1], vec![0u8; 1]).unwrap();
    /// assert!(!block.is_empty());
    /// let empty = VoxelBlock::<u8>::new([0, 0, 0], [0, 0, 0], vec![]).unwrap();
    /// assert!(empty.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns `true` if this block covers the entire volume starting at the origin.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::{VoxelBlock, VolumeShape};
    /// let vol = VolumeShape::new(4, 4, 4);
    /// let full = VoxelBlock::new([0, 0, 0], [4, 4, 4], vec![0u8; 64]).unwrap();
    /// assert!(full.is_full_volume(&vol));
    /// let partial = VoxelBlock::new([1, 0, 0], [3, 4, 4], vec![0u8; 48]).unwrap();
    /// assert!(!partial.is_full_volume(&vol));
    /// ```
    pub fn is_full_volume(&self, volume_shape: &VolumeShape) -> bool {
        self.offset == [0, 0, 0]
            && self.shape == [volume_shape.nx, volume_shape.ny, volume_shape.nz]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Header;

    #[test]
    fn volume_shape_from_header_ok() {
        let mut h = Header::new();
        h.nx = 64;
        h.ny = 128;
        h.nz = 256;
        let vs = VolumeShape::from_header(&h).unwrap();
        assert_eq!(vs.nx, 64);
        assert_eq!(vs.ny, 128);
        assert_eq!(vs.nz, 256);
    }

    #[test]
    fn volume_shape_from_header_negative_nx() {
        let mut h = Header::new();
        h.nx = -1;
        assert!(VolumeShape::from_header(&h).is_err());
    }

    #[test]
    fn volume_shape_from_header_negative_ny() {
        let mut h = Header::new();
        h.ny = -100;
        assert!(VolumeShape::from_header(&h).is_err());
    }

    #[test]
    fn volume_shape_from_header_negative_nz() {
        let mut h = Header::new();
        h.nz = -1;
        assert!(VolumeShape::from_header(&h).is_err());
    }

    #[test]
    fn volume_shape_empty() {
        let vs = VolumeShape::new(0, 0, 0);
        assert!(vs.is_empty());
    }

    #[test]
    fn volume_shape_contains_block() {
        let vs = VolumeShape::new(10, 10, 10);
        assert!(vs.contains_block([0, 0, 0], [5, 5, 5]));
        assert!(vs.contains_block([5, 5, 5], [5, 5, 5]));
        assert!(!vs.contains_block([0, 0, 0], [11, 1, 1]));
        assert!(!vs.contains_block([0, 0, 0], [1, 11, 1]));
        assert!(!vs.contains_block([0, 0, 0], [1, 1, 11]));
    }

    #[test]
    fn volume_shape_checked_linear_index() {
        let vs = VolumeShape::new(4, 3, 2);
        // origin
        assert_eq!(vs.checked_linear_index([0, 0, 0]), Some(0));
        // last voxel
        assert_eq!(vs.checked_linear_index([3, 2, 1]), Some(4 * 3 * 2 - 1));
        // first row, first slice
        assert_eq!(vs.checked_linear_index([2, 1, 0]), Some(6));
        // second slice
        assert_eq!(vs.checked_linear_index([0, 0, 1]), Some(12));
        // overflow should return None
        assert_eq!(vs.checked_linear_index([0, 0, usize::MAX]), None);
    }

    #[test]
    fn voxel_block_shape_mismatch() {
        let err = VoxelBlock::<f32>::new([0, 0, 0], [2, 2, 2], vec![0.0f32; 5]).unwrap_err();
        assert!(matches!(err, crate::Error::BlockShapeMismatch { .. }));
    }

    #[test]
    fn voxel_block_is_full_volume() {
        let vs = VolumeShape::new(4, 4, 4);
        let block = VoxelBlock::new([0, 0, 0], [4, 4, 4], vec![0.0f32; 64]).unwrap();
        assert!(block.is_full_volume(&vs));
        let offset_block = VoxelBlock::new([1, 0, 0], [3, 4, 4], vec![0.0f32; 48]).unwrap();
        assert!(!offset_block.is_full_volume(&vs));
    }
}
