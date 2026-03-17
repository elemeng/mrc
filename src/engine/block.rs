//! Voxel block types

use alloc::vec::Vec;

/// Volume geometry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeShape {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

impl VolumeShape {
    pub const fn new(nx: usize, ny: usize, nz: usize) -> Self {
        Self { nx, ny, nz }
    }

    pub const fn total_voxels(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    pub const fn is_empty(&self) -> bool {
        self.nx == 0 || self.ny == 0 || self.nz == 0
    }

    /// Check if a block with given offset and shape fits within this volume.
    /// Returns true if the block is completely within bounds.
    pub fn contains_block(&self, offset: [usize; 3], shape: [usize; 3]) -> bool {
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;
        ox + sx <= self.nx && oy + sy <= self.ny && oz + sz <= self.nz
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
        assert_eq!(
            data.len(),
            shape[0] * shape[1] * shape[2],
            "Data length must match block shape"
        );
        Self {
            offset,
            shape,
            data,
        }
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
/// The caller must ensure the type T matches the file's voxel mode exactly.
/// Using the wrong type will produce incorrect data without any runtime error.
/// For type-safe access, use `write_block` instead.
pub trait SliceAccess {
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
