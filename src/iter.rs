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
    volume_shape: VolumeShape,
    stepper: S,
}

impl<'a, S> RegionIter<'a, S> {
    pub(crate) fn with_stepper(reader: &'a Reader, volume_shape: VolumeShape, stepper: S) -> Self {
        Self {
            reader,
            volume_shape,
            stepper,
        }
    }

    /// Try zero-copy reinterpretation for a native-endian contiguous slab.
    /// Returns `Some(DataView)` on success, `None` if the block cannot be
    /// zero-copied (non-contiguous, endian mismatch, or alignment issue).
    pub(crate) fn try_zero_copy<'b>(bytes: &'b [u8], mode: Mode) -> Option<DataView<'b>> {
        Some(match mode {
            Mode::Int8 => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<i8>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Int8(data)
            }
            Mode::Int16 => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<i16>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Int16(data)
            }
            Mode::Float32 => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<f32>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Float32(data)
            }
            Mode::Int16Complex => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<crate::Int16Complex>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Int16Complex(data)
            }
            Mode::Float32Complex => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<crate::Float32Complex>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Float32Complex(data)
            }
            Mode::Uint16 => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<u16>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Uint16(data)
            }
            #[cfg(feature = "f16")]
            Mode::Float16 => {
                let (prefix, data, _suffix) = unsafe { bytes.align_to::<crate::f16>() };
                if !prefix.is_empty() {
                    return None;
                }
                DataView::Float16(data)
            }
            Mode::Packed4Bit => DataView::Packed4Bit(bytes),
        })
    }
}

impl<'a, S: Stepper> Iterator for RegionIter<'a, S> {
    type Item = Result<DataBlock<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, shape) = self.stepper.next(self.volume_shape)?;
        let bytes = match self.reader.read_block_bytes_cow(offset, shape) {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };

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
            match decode_block_to_any(&bytes, self.reader.mode(), self.reader.endian(), shape) {
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
