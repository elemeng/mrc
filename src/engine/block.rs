//! Voxel block types

use std::vec::Vec;

/// Volume geometry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VolumeShape {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

impl VolumeShape {
    pub const fn new(nx: usize, ny: usize, nz: usize) -> Self {
        Self { nx, ny, nz }
    }

    pub fn total_voxels(&self) -> Option<usize> {
        self.nx.checked_mul(self.ny)?.checked_mul(self.nz)
    }

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

/// Universal representation of voxel chunks
#[derive(Debug, Clone)]
pub struct VoxelBlock<T> {
    pub offset: [usize; 3],
    pub shape: [usize; 3],
    pub data: Vec<T>,
}

impl<T> VoxelBlock<T> {
    pub fn new(offset: [usize; 3], shape: [usize; 3], data: Vec<T>) -> Self {
        let expected = shape[0].checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .expect("Block shape dimensions overflow usize");
        assert_eq!(
            data.len(), expected,
            "Data length must match block shape"
        );
        Self {
            offset,
            shape,
            data,
        }
    }

    /// Create a new VoxelBlock, returning an error if the data length does not match the shape.
    pub fn try_new(
        offset: [usize; 3],
        shape: [usize; 3],
        data: Vec<T>,
    ) -> Result<Self, crate::Error> {
        let expected = shape[0].checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or(crate::Error::BoundsError)?;
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

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn is_full_volume(&self, volume_shape: &VolumeShape) -> bool {
        self.offset == [0, 0, 0]
            && self.shape == [volume_shape.nx, volume_shape.ny, volume_shape.nz]
    }
}

/// Trait for writers that provide direct slice access to voxel data.
///
/// # Safety Warning
/// The caller must ensure:
/// - The type `T` matches the file's voxel mode exactly.
/// - The file's data offset is aligned for `T` (always true for files created
///   by this crate, but may not hold for arbitrary third-party files).
///
/// Violating either precondition causes undefined behaviour.
/// For type-safe access, use `write_block` instead.
pub trait SliceAccess {
    /// Get an immutable slice of voxels at the given z-index.
    fn slice<T: crate::engine::codec::EndianCodec>(
        &self,
        z: usize,
    ) -> Result<&[T], crate::Error>;

    /// Get a mutable slice of voxels at the given z-index.
    ///
    /// # Example
    /// ```ignore
    /// // Correct: file was created with mode f32
    /// let slice = writer.slice_mut::<f32>(0)?;
    ///
    /// // Wrong: undefined behavior if file mode is not i16
    /// let slice = writer.slice_mut::<i16>(0)?; // Don't do this!
    /// ```
    fn slice_mut<T: crate::engine::codec::EndianCodec>(
        &mut self,
        z: usize,
    ) -> Result<&mut [T], crate::Error>;
}
