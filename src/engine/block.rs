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
