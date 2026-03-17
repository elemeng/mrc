//! Iterator engine for reading MRC files

use crate::Error;
use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::{EndianCodec, DefaultValue};

pub struct SliceIter<'a, T> {
    reader: &'a crate::Reader,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> SliceIter<'a, T> {
    pub fn new(reader: &'a crate::Reader, shape: VolumeShape) -> Self {
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

impl<'a, T> Iterator for SliceIter<'a, T>
where
    T: EndianCodec + Send + Copy + DefaultValue,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let result = self
            .reader
            .read_voxels([0, 0, self.z], [self.nx, self.ny, 1]);
        self.z += 1;

        match result {
            Ok(bytes) => {
                let decoded = match self.reader.decode_block::<T>(&bytes) {
                    Ok(data) => data,
                    Err(e) => return Some(Err(e)),
                };

                let block = VoxelBlock {
                    offset: [0, 0, self.z - 1],
                    shape: [self.nx, self.ny, 1],
                    data: decoded,
                };
                Some(Ok(block))
            }
            Err(e) => Some(Err(e)),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = n;
        self.next()
    }
}

pub struct SlabIter<'a, T> {
    reader: &'a crate::Reader,
    z: usize,
    nz: usize,
    nx: usize,
    ny: usize,
    slab_size: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> SlabIter<'a, T> {
    pub fn new(reader: &'a crate::Reader, shape: VolumeShape, k: usize) -> Self {
        Self {
            reader,
            z: 0,
            nz: shape.nz,
            nx: shape.nx,
            ny: shape.ny,
            slab_size: k,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for SlabIter<'a, T>
where
    T: EndianCodec + Send + Copy + DefaultValue,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.z >= self.nz {
            return None;
        }

        let remaining = self.nz - self.z;
        let size = self.slab_size.min(remaining);

        let result = self
            .reader
            .read_voxels([0, 0, self.z], [self.nx, self.ny, size]);
        self.z += size;

        match result {
            Ok(bytes) => {
                let decoded = match self.reader.decode_block::<T>(&bytes) {
                    Ok(data) => data,
                    Err(e) => return Some(Err(e)),
                };

                let block = VoxelBlock {
                    offset: [0, 0, self.z - size],
                    shape: [self.nx, self.ny, size],
                    data: decoded,
                };
                Some(Ok(block))
            }
            Err(e) => Some(Err(e)),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.z = n * self.slab_size;
        self.next()
    }
}

pub struct BlockIter<'a, T> {
    reader: &'a crate::Reader,
    position: [usize; 3],
    shape: VolumeShape,
    chunk_shape: [usize; 3],
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T> BlockIter<'a, T> {
    pub fn new(reader: &'a crate::Reader, shape: VolumeShape, chunk_shape: [usize; 3]) -> Self {
        Self {
            reader,
            position: [0, 0, 0],
            shape,
            chunk_shape,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for BlockIter<'a, T>
where
    T: EndianCodec + Send + Copy + DefaultValue,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [cx, cy, cz] = self.chunk_shape;
        let [px, py, pz] = self.position;

        if pz >= nz {
            return None;
        }

        let sx = cx.min(nx - px);
        let sy = cy.min(ny - py);
        let sz = cz.min(nz - pz);

        let result = self.reader.read_voxels([px, py, pz], [sx, sy, sz]);

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

        match result {
            Ok(bytes) => {
                let decoded = match self.reader.decode_block::<T>(&bytes) {
                    Ok(data) => data,
                    Err(e) => return Some(Err(e)),
                };

                let block = VoxelBlock {
                    offset: [px, py, pz],
                    shape: [sx, sy, sz],
                    data: decoded,
                };
                Some(Ok(block))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
