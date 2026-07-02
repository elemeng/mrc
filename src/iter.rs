//! Lazy iterators for reading MRC files by slices, slabs, or blocks.
//!
//! All iterators are backed by a single [`RegionIter`] type parameterized by a
//! [`Stepper`] strategy that generates `(offset, shape)` pairs. The concrete
//! stepper types — [`SliceStepper`], [`SlabStepper`], [`TileStepper`] — define
//! how the volume is partitioned.
//!
//! Users typically obtain iterators from reader methods rather than constructing
//! them directly:
//!
//! - [`Reader::slices`](crate::Reader::slices) — one Z-plane at a time
//! - [`Reader::slabs`](crate::Reader::slabs) — batches of `k` Z-planes
//! - [`Reader::tiles`](crate::Reader::tiles) — arbitrary 3D tiles
//! - [`reader.convert::<f32>().slices()`](crate::io::reader_common::ConvertReader::slices) — any mode auto-converted to `f32`

use crate::Error;
use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::io::reader_common::VoxelSource;
use crate::mode::Voxel;

// ============================================================================
// Stepper trait – generates (offset, shape) sequences
// ============================================================================

/// Strategy for stepping through a volume as a sequence of blocks.
///
/// This trait is `#[doc(hidden)]` — users interact with the concrete
/// [`SliceStepper`], [`SlabStepper`], and [`TileStepper`] types instead.
#[doc(hidden)]
pub trait Stepper {
    /// Returns the next `(offset, shape)` pair, or `None` when exhausted.
    fn next(&mut self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])>;
}

/// Step one Z-plane at a time (`[nx, ny, 1]`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SliceStepper {
    z: usize,
}

impl SliceStepper {
    /// Create a new slice stepper that yields one Z-plane per step.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Stepper for SliceStepper {
    fn next(&mut self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])> {
        if self.z >= volume_shape.nz {
            return None;
        }
        let z = self.z;
        self.z += 1;
        Some(([0, 0, z], [volume_shape.nx, volume_shape.ny, 1]))
    }
}

/// Step `k` contiguous Z-slices at a time (`[nx, ny, k]`).
#[derive(Debug, Clone, Copy)]
pub struct SlabStepper {
    z: usize,
    k: usize,
}

impl SlabStepper {
    /// Create a new slab stepper that yields `k` contiguous Z-planes per step.
    ///
    /// `k` is clamped to at least 1. The final slab may be shorter near the
    /// end of the volume.
    pub fn new(k: usize) -> Self {
        Self { z: 0, k: k.max(1) }
    }
}

impl Stepper for SlabStepper {
    fn next(&mut self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])> {
        if self.z >= volume_shape.nz {
            return None;
        }
        let z = self.z;
        let sz = self.k.min(volume_shape.nz - z);
        self.z += sz;
        Some(([0, 0, z], [volume_shape.nx, volume_shape.ny, sz]))
    }
}

/// Step arbitrary 3D tiles across a volume.
#[derive(Debug, Clone, Copy)]
pub struct TileStepper {
    position: [usize; 3],
    tile_shape: [usize; 3],
}

impl TileStepper {
    /// Create a new tile stepper that partitions the volume into tiles of the
    /// given shape.
    ///
    /// # Panics
    /// Panics if any dimension of `tile_shape` is zero.
    pub fn new(tile_shape: [usize; 3]) -> Self {
        assert!(
            tile_shape[0] > 0 && tile_shape[1] > 0 && tile_shape[2] > 0,
            "tile_shape must be positive in all dimensions"
        );
        Self {
            position: [0, 0, 0],
            tile_shape,
        }
    }
}

impl Stepper for TileStepper {
    fn next(&mut self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])> {
        let [nx, ny, nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
        let [cx, cy, cz] = self.tile_shape;
        let [px, py, pz] = self.position;

        if pz >= nz {
            return None;
        }

        let sx = cx.min(nx - px);
        let sy = cy.min(ny - py);
        let sz = cz.min(nz - pz);

        // Advance position
        self.position[0] += cx;
        if self.position[0] >= nx {
            self.position[0] = 0;
            self.position[1] += cy;
            if self.position[1] >= ny {
                self.position[1] = 0;
                self.position[2] += cz;
            }
        }

        Some(([px, py, pz], [sx, sy, sz]))
    }
}

// ============================================================================
// RegionIter – unified block iterator
// ============================================================================

/// Lazy iterator over a volume as a sequence of [`VoxelBlock`]s.
///
/// The stepping strategy (slices, slabs, tiles) is determined by the `S`
/// type parameter.
#[derive(Debug)]
pub struct RegionIter<'a, T, R, S> {
    reader: &'a R,
    volume_shape: VolumeShape,
    stepper: S,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T, R, S> RegionIter<'a, T, R, S> {
    /// Create a new region iterator with an explicit stepper.
    ///
    /// Prefer using the convenience methods on [`Reader`](crate::Reader) and
    /// [`MmapReader`](crate::MmapReader) (`slices`, `slabs`, `tiles`, etc.)
    /// which construct the appropriate stepper automatically.
    pub fn with_stepper(reader: &'a R, volume_shape: VolumeShape, stepper: S) -> Self {
        Self {
            reader,
            volume_shape,
            stepper,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T, R, S> Iterator for RegionIter<'a, T, R, S>
where
    R: VoxelSource,
    S: Stepper,
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, shape) = self.stepper.next(self.volume_shape)?;
        let bytes = match self.reader.vs_read_block_bytes(offset, shape) {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };
        let data = match self.reader.vs_decode_block::<T>(&bytes) {
            Ok(d) => d,
            Err(e) => return Some(Err(e)),
        };
        Some(Ok(VoxelBlock {
            offset,
            shape,
            data,
        }))
    }
}

impl<'a, T, R, S> core::iter::FusedIterator for RegionIter<'a, T, R, S>
where
    R: VoxelSource,
    S: Stepper,
    T: Voxel,
{
}
