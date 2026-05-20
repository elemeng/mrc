//! Iterator engine for reading MRC files

use crate::Error;
use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::mode::Voxel;
use crate::reader_common::VoxelSource;

/// Helper to read and decode a voxel block from a VoxelSource.
#[inline]
fn read_and_decode<T: Voxel>(
    reader: &impl VoxelSource,
    offset: [usize; 3],
    shape: [usize; 3],
) -> Result<VoxelBlock<T>, Error> {
    let bytes = reader.vs_read_block_bytes(offset, shape)?;
    let data = reader.vs_decode_block::<T>(&bytes)?;
    Ok(VoxelBlock {
        offset,
        shape,
        data,
    })
}

#[derive(Debug)]
pub struct SliceIter<'a, T, R: VoxelSource> {
    reader: &'a R,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T, R: VoxelSource> SliceIter<'a, T, R> {
    pub fn new(reader: &'a R, shape: VolumeShape) -> Self {
        Self {
            reader,
            z: 0,
            nz: shape.nz,
            nx: shape.nx,
            ny: shape.ny,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T, R: VoxelSource> Iterator for SliceIter<'a, T, R>
where
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let z = self.z;
        self.z += 1;

        Some(read_and_decode(
            self.reader,
            [0, 0, z],
            [self.nx, self.ny, 1],
        ))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = self.z.saturating_add(n);
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nz.saturating_sub(self.z);
        (remaining, Some(remaining))
    }
}

impl<'a, T, R: VoxelSource> ExactSizeIterator for SliceIter<'a, T, R> where T: Voxel {}
impl<'a, T, R: VoxelSource> core::iter::FusedIterator for SliceIter<'a, T, R> where T: Voxel {}

#[derive(Debug)]
pub struct SlabIter<'a, T, R: VoxelSource> {
    reader: &'a R,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    slab_size: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T, R: VoxelSource> SlabIter<'a, T, R> {
    pub fn new(reader: &'a R, shape: VolumeShape, k: usize) -> Self {
        Self {
            reader,
            z: 0,
            nz: shape.nz,
            nx: shape.nx,
            ny: shape.ny,
            slab_size: k.max(1),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T, R: VoxelSource> Iterator for SlabIter<'a, T, R>
where
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let z = self.z;
        let size = self.slab_size.min(self.nz - z);
        self.z += size;

        Some(read_and_decode(
            self.reader,
            [0, 0, z],
            [self.nx, self.ny, size],
        ))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = self.z.saturating_add(n.saturating_mul(self.slab_size));
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nz.saturating_sub(self.z);
        let count = remaining.div_ceil(self.slab_size);
        (count, Some(count))
    }
}

impl<'a, T, R: VoxelSource> ExactSizeIterator for SlabIter<'a, T, R> where T: Voxel {}
impl<'a, T, R: VoxelSource> core::iter::FusedIterator for SlabIter<'a, T, R> where T: Voxel {}

#[derive(Debug)]
pub struct BlockIter<'a, T, R: VoxelSource> {
    reader: &'a R,
    position: [usize; 3],
    shape: VolumeShape,
    block_shape: [usize; 3],
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T, R: VoxelSource> BlockIter<'a, T, R> {
    pub fn new(reader: &'a R, shape: VolumeShape, block_shape: [usize; 3]) -> Self {
        assert!(block_shape[0] > 0 && block_shape[1] > 0 && block_shape[2] > 0, "block_shape must be positive in all dimensions");
        Self {
            reader,
            position: [0, 0, 0],
            shape,
            block_shape,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T, R: VoxelSource> Iterator for BlockIter<'a, T, R>
where
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [cx, cy, cz] = self.block_shape;
        let [px, py, pz] = self.position;

        if pz >= nz {
            return None;
        }

        let sx = cx.min(nx - px);
        let sy = cy.min(ny - py);
        let sz = cz.min(nz - pz);

        // Update position
        self.position[0] += cx;
        if self.position[0] >= nx {
            self.position[0] = 0;
            self.position[1] += cy;
            if self.position[1] >= ny {
                self.position[1] = 0;
                self.position[2] += cz;
            }
        }

        Some(read_and_decode(self.reader, [px, py, pz], [sx, sy, sz]))
    }
}

impl<'a, T, R: VoxelSource> core::iter::FusedIterator for BlockIter<'a, T, R> where T: Voxel {}
