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

use crate::Error;
use crate::Reader;
use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::mode::Voxel;

// ============================================================================
// Stepper trait – generates (offset, shape) sequences
// ============================================================================

/// Strategy for stepping through a volume as a sequence of blocks.
#[doc(hidden)]
pub trait Stepper {
    /// Returns the next `(offset, shape)` pair, or `None` when exhausted.
    fn next(&mut self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])>;
}

/// Step one Z-plane at a time (`[nx, ny, 1]`).
///
/// # Examples
///
/// ```rust
/// use mrc::SliceStepper;
/// let stepper = SliceStepper::default();
/// assert_eq!(format!("{:?}", stepper), "SliceStepper { z: 0 }");
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct SliceStepper {
    z: usize,
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
///
/// # Examples
///
/// ```rust
/// use mrc::SlabStepper;
/// let stepper = SlabStepper::new(16);
/// assert_eq!(format!("{:?}", stepper), "SlabStepper { z: 0, k: 16 }");
/// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::SlabStepper;
    /// let stepper = SlabStepper::new(10);
    /// assert_eq!(format!("{:?}", stepper), "SlabStepper { z: 0, k: 10 }");
    /// // k=0 is clamped to 1
    /// let clamped = SlabStepper::new(0);
    /// assert_eq!(format!("{:?}", clamped), "SlabStepper { z: 0, k: 1 }");
    /// ```
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
///
/// # Examples
///
/// ```rust
/// use mrc::TileStepper;
/// let stepper = TileStepper::new([64, 64, 64]).unwrap();
/// assert_eq!(
///     format!("{:?}", stepper),
///     "TileStepper { position: [0, 0, 0], tile_shape: [64, 64, 64] }"
/// );
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TileStepper {
    position: [usize; 3],
    tile_shape: [usize; 3],
}

impl TileStepper {
    /// Create a new tile stepper that partitions the volume into tiles of the
    /// given shape.
    ///
    /// # Errors
    /// Returns [`crate::Error::BoundsError`] if any dimension of `tile_shape` is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::TileStepper;
    /// let stepper = TileStepper::new([64, 64, 64])?;
    /// assert_eq!(
    ///     format!("{:?}", stepper),
    ///     "TileStepper { position: [0, 0, 0], tile_shape: [64, 64, 64] }"
    /// );
    /// // Zero dimensions are rejected
    /// assert!(TileStepper::new([0, 64, 64]).is_err());
    /// # Ok::<_, mrc::Error>(())
    /// ```
    pub fn new(tile_shape: [usize; 3]) -> Result<Self, crate::Error> {
        if tile_shape[0] == 0 || tile_shape[1] == 0 || tile_shape[2] == 0 {
            return Err(crate::Error::bounds_err());
        }
        Ok(Self {
            position: [0, 0, 0],
            tile_shape,
        })
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
// RegionIter – unified block iterator over a Reader
// ============================================================================

/// Lazy iterator over a volume as a sequence of [`VoxelBlock`]s.
///
/// The stepping strategy (slices, slabs, tiles) is determined by the `S`
/// type parameter.
///
/// # Examples
///
/// ```no_run
/// use mrc::open;
///
/// let reader = open("density.mrc")?;
/// for slice in reader.slices::<f32>() {
///     let block = slice?;
///     println!("z={} shape={:?}x{:?}x{:?}",
///         block.offset[2], block.shape[0], block.shape[1], block.shape[2]);
/// }
/// # Ok::<_, mrc::Error>(())
/// ```
#[derive(Debug)]
pub struct RegionIter<'a, T, S> {
    reader: &'a Reader,
    volume_shape: VolumeShape,
    stepper: S,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T, S> RegionIter<'a, T, S> {
    /// Create a new region iterator with an explicit stepper.
    pub fn with_stepper(reader: &'a Reader, volume_shape: VolumeShape, stepper: S) -> Self {
        Self {
            reader,
            volume_shape,
            stepper,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a, T, S> Iterator for RegionIter<'a, T, S>
where
    S: Stepper,
    T: Voxel,
{
    type Item = Result<VoxelBlock<T>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, shape) = self.stepper.next(self.volume_shape)?;
        let bytes = match self.reader.read_block_bytes_cow(offset, shape) {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };
        let data = match self.reader.decode_block::<T>(&bytes) {
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

impl<'a, T, S> core::iter::FusedIterator for RegionIter<'a, T, S>
where
    S: Stepper,
    T: Voxel,
{
}
