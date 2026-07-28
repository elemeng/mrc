//! Lazy iterators for reading MRC files by slices, slabs, or blocks.
//!
//! All iterators are backed by a single [`RegionIter`] type parameterized by a
//! [`Stepper`] strategy that generates `(offset, shape)` pairs. The concrete
//! stepper types — [`SliceStepper`], [`SlabStepper`], [`TileStepper`] — define
//! how the volume is partitioned.
//!
//! These types are internal implementation details. Users obtain iterators from
//! reader methods such as [`slices`](crate::Reader::slices).

use crate::Error;
use crate::Reader;
use crate::engine::block::VolumeShape;
use crate::engine::convert::decode_block_to_any;
use crate::mode::{DataBlock, DataView, Mode};
use std::borrow::Cow;

// ============================================================================
// Stepper trait – generates (offset, shape) sequences
// ============================================================================

/// Strategy for stepping through a volume as a sequence of blocks.
pub(crate) trait Stepper {
    /// Returns the next `(offset, shape)` pair, or `None` when exhausted.
    fn next(&mut self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])>;

    /// Returns the next `(offset, shape)` pair **without** advancing state,
    /// or `None` when exhausted.
    fn peek(&self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])>;
}

/// Step one Z-plane at a time (`[nx, ny, 1]`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SliceStepper {
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

    fn peek(&self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])> {
        if self.z >= volume_shape.nz {
            return None;
        }
        Some(([0, 0, self.z], [volume_shape.nx, volume_shape.ny, 1]))
    }
}

/// Step `k` contiguous Z-slices at a time (`[nx, ny, k]`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SlabStepper {
    z: usize,
    k: usize,
}

impl SlabStepper {
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

    fn peek(&self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])> {
        if self.z >= volume_shape.nz {
            return None;
        }
        let sz = self.k.min(volume_shape.nz - self.z);
        Some(([0, 0, self.z], [volume_shape.nx, volume_shape.ny, sz]))
    }
}

/// Step arbitrary 3D tiles across a volume.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TileStepper {
    position: [usize; 3],
    tile_shape: [usize; 3],
}

impl TileStepper {
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

    fn peek(&self, volume_shape: VolumeShape) -> Option<([usize; 3], [usize; 3])> {
        TileStepper::peek_at(self.position, self.tile_shape, volume_shape)
    }
}

impl TileStepper {
    #[inline]
    fn peek_at(
        position: [usize; 3],
        tile_shape: [usize; 3],
        volume_shape: VolumeShape,
    ) -> Option<([usize; 3], [usize; 3])> {
        let [nx, ny, nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
        let [cx, cy, cz] = tile_shape;
        let [px, py, pz] = position;

        if pz >= nz {
            return None;
        }

        let sx = cx.min(nx - px);
        let sy = cy.min(ny - py);
        let sz = cz.min(nz - pz);

        Some(([px, py, pz], [sx, sy, sz]))
    }
}

// ============================================================================
// RegionIter – unified block iterator over a Reader
// ============================================================================

/// Lazy iterator over a volume as a sequence of [`DataBlock`]s.
///
/// The stepping strategy (slices, slabs, tiles) is determined by the `S`
/// type parameter.
///
/// This type is internal. Users obtain iterators via reader methods such as
/// [`Reader::slices`](crate::Reader::slices).
#[derive(Debug)]
pub(crate) struct RegionIter<'a, S> {
    reader: &'a Reader,
    stepper: S,
}

impl<'a, S> RegionIter<'a, S> {
    pub(crate) fn with_stepper(reader: &'a Reader, stepper: S) -> Self {
        Self { reader, stepper }
    }

    fn shape(&self) -> VolumeShape {
        self.reader.shape()
    }

    /// Try zero-copy reinterpretation for a native-endian contiguous slab.
    /// Returns `Some(DataView)` on success, `None` if the block cannot be
    /// zero-copied (non-contiguous, endian mismatch, or alignment issue).
    /// Try to reinterpret bytes as a typed slice via `align_to`.
    /// Returns `None` if `bytes` is not correctly aligned for `T`.
    fn try_align<T>(bytes: &[u8]) -> Option<&[T]> {
        let (prefix, data, suffix) = unsafe { bytes.align_to::<T>() };
        if prefix.is_empty() && suffix.is_empty() { Some(data) } else { None }
    }

    pub(crate) fn try_zero_copy<'b>(bytes: &'b [u8], mode: Mode) -> Option<DataView<'b>> {
        Some(match mode {
            Mode::Int8 => DataView::Int8(Self::try_align(bytes)?),
            Mode::Int16 => DataView::Int16(Self::try_align(bytes)?),
            Mode::Float32 => DataView::Float32(Self::try_align(bytes)?),
            Mode::Uint16 => DataView::Uint16(Self::try_align(bytes)?),
            Mode::Int16Complex => DataView::Int16Complex(Self::try_align(bytes)?),
            Mode::Float32Complex => DataView::Float32Complex(Self::try_align(bytes)?),
            #[cfg(feature = "f16")]
            Mode::Float16 => DataView::Float16(Self::try_align(bytes)?),
            #[cfg(not(feature = "f16"))]
            Mode::Float16 => return None,
            Mode::Packed4Bit => DataView::Packed4Bit(bytes),
        })
    }
}

impl<'a, S: Stepper> Iterator for RegionIter<'a, S> {
    type Item = Result<DataBlock<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, shape) = self.stepper.peek(self.shape())?;
        let bytes = match self.reader.read_block_bytes_cow(offset, shape) {
            Ok(b) => b,
            Err(e) => {
                self.stepper.next(self.shape());
                return Some(Err(e));
            }
        };

        // Commit: advance stepper only after successful read
        self.stepper.next(self.shape());

        // Zero-copy path: native endian + contiguous block (Cow::Borrowed)
        if self.reader.endian().is_native() {
            if let Cow::Borrowed(b) = &bytes {
                if let Some(data) = Self::try_zero_copy(b, self.reader.mode()) {
                    return Some(Ok(DataBlock::Borrowed {
                        offset,
                        shape,
                        data,
                    }));
                }
            }
        }

        // One-copy path: decode to owned data
        let data =
            match decode_block_to_any(&bytes, self.reader.mode(), self.reader.endian()) {
                Ok(d) => d,
                Err(e) => return Some(Err(e)),
            };
        Some(Ok(DataBlock::Owned {
            offset,
            shape,
            data,
        }))
    }
}

impl<'a, S> core::iter::FusedIterator for RegionIter<'a, S> where S: Stepper {}
