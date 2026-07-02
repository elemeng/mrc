//! Shared helpers for all MRC reader implementations.
//!
//! This module contains the [`VoxelSource`] trait and helper functions that are
//! used by [`Reader`](crate::Reader) and [`MmapReader`](crate::MmapReader) to implement block validation, endian
//! decoding, and the `slices_f32` / `slabs_f32` convenience iterators.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::{EndianCodec, decode_slice, encode_slice};
use crate::engine::endian::FileEndian;
use crate::iter::{RegionIter, SlabStepper, SliceStepper, TileStepper};
use crate::mode::{ComplexToRealStrategy, Float32Complex, Int16Complex, M0Interpretation, Voxel};
use crate::{Error, Header, Mode};
use std::borrow::Cow;
use std::io::Read;

/// Internal helper: boxed iterator over [`VoxelBlock`] results.
///
/// The most common pattern for conversion iterators that mix heterogeneous
/// inner iterator types (e.g. `slices_f32` branching on mode). Defining it
/// as a type alias avoids clippy's `type_complexity` lint while keeping the
/// concrete type visible.
type VoxelIter<'a, T> = Box<dyn Iterator<Item = Result<VoxelBlock<T>, Error>> + 'a>;

mod private {
    /// Sealed trait marker — prevents external implementations of [`VoxelSource`].
    pub trait Sealed {}
}

/// Sealed trait unifying all reader types for generic iterators.
///
/// [`RegionIter`](crate::RegionIter) is generic over `VoxelSource` so it can
/// work with any reader backend without monomorphising on the concrete type.
///
/// This trait is sealed: it can only be implemented by types inside this crate.
#[doc(hidden)]
pub trait VoxelSource: private::Sealed {
    /// Read a block of raw bytes from the data region.
    ///
    /// Returns `Cow::Borrowed` for zero-copy backends (e.g. [`MmapReader`](crate::MmapReader))
    /// and `Cow::Owned` for in-memory backends.
    fn vs_read_block_bytes<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Cow<'a, [u8]>, Error>;

    /// Decode raw bytes to the requested voxel type, checking mode compatibility.
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error>;
}

/// Core metadata accessors shared by all reader types.
///
/// This trait is `#[doc(hidden)]` — it powers the iterator system internally.
///
/// `dead_code` is suppressed because the trait is used implicitly through
/// inherent method resolution (the `impl_inherent_reader_methods!` macro
/// calls `self.shape()` etc. which resolve through `ReaderCore`).
#[doc(hidden)]
#[allow(dead_code)]
pub trait ReaderCore: VoxelSource {
    /// Volume dimensions in voxels.
    fn shape(&self) -> VolumeShape;

    /// Voxel data mode.
    fn mode(&self) -> Mode;

    /// Detected file endianness.
    fn endian(&self) -> FileEndian;

    /// Reference to the parsed header.
    fn header(&self) -> &Header;

    /// Extended header bytes (empty slice if no extended header).
    fn ext_header_bytes(&self) -> &[u8];
}

/// Generates inherent iterator methods on reader types.
macro_rules! impl_inherent_reader_methods {
    ($ty:ty) => {
        impl $ty {
            /// Iterate over Z-slices (1 voxel thick along Z) as [`VoxelBlock`]s.
            ///
            /// Each item is a contiguous full-XY slab at one Z position.
            /// See also [`slices_f32`](Self::slices_f32) for automatic mode conversion.
            pub fn slices<T: Voxel>(&self) -> RegionIter<'_, T, $ty, SliceStepper> {
                RegionIter::with_stepper(self, self.shape(), SliceStepper::new())
            }
            /// Iterate over Z-slabs of `k` slices as [`VoxelBlock`]s.
            ///
            /// Each item is a contiguous full-XY slab of `k` Z-planes.
            /// The final slab may be shorter than `k` near the end of the volume.
            /// See also [`slabs_f32`](Self::slabs_f32) for automatic mode conversion.
            pub fn slabs<T: Voxel>(&self, k: usize) -> RegionIter<'_, T, $ty, SlabStepper> {
                RegionIter::with_stepper(self, self.shape(), SlabStepper::new(k))
            }
            /// Iterate over 3D tiles of the given shape as [`VoxelBlock`]s.
            ///
            /// The volume is partitioned into non-overlapping tiles of size
            /// `tile_shape`. Tiles at the trailing edges may be truncated to fit
            /// the volume bounds.
            pub fn tiles<T: Voxel>(
                &self,
                tile_shape: [usize; 3],
            ) -> RegionIter<'_, T, $ty, TileStepper> {
                RegionIter::with_stepper(self, self.shape(), TileStepper::new(tile_shape))
            }
            /// Alias for [`slices`](Self::slices).
            pub fn images<T: Voxel>(&self) -> RegionIter<'_, T, $ty, SliceStepper> {
                self.slices()
            }
            /// Alias for [`slabs`](Self::slabs).
            pub fn image_stack<T: Voxel>(&self, k: usize) -> RegionIter<'_, T, $ty, SlabStepper> {
                self.slabs(k)
            }
            /// Alias for [`slices`](Self::slices).
            pub fn planes<T: Voxel>(&self) -> RegionIter<'_, T, $ty, SliceStepper> {
                self.slices()
            }
            /// Alias for [`slabs`](Self::slabs).
            pub fn plane_stack<T: Voxel>(&self, k: usize) -> RegionIter<'_, T, $ty, SlabStepper> {
                self.slabs(k)
            }
            /// Iterate over sub-volumes of a volume-stack file.
            ///
            /// Each sub-volume is `mz` slices thick, where `mz` is taken from
            /// the header's sampling field.
            ///
            /// # Errors
            /// Returns [`Error::NotAVolumeStack`] if the file is not a volume stack
            /// (ispg not in 401–630).
            pub fn volumes<T: Voxel>(&self) -> Result<RegionIter<'_, T, $ty, SlabStepper>, Error> {
                let mz = self.header().mz.max(0) as usize;
                if !self.header().is_volume_stack() || mz == 0 {
                    return Err(Error::NotAVolumeStack {
                        ispg: self.header().ispg,
                        mz: self.header().mz,
                    });
                }
                Ok(self.slabs(mz))
            }
            /// Read a single arbitrary 3D sub-region as a [`VoxelBlock`].
            ///
            /// Unlike the iterators (`slices`, `slabs`, `tiles`), this reads
            /// exactly one block at the given offset and shape. Useful for
            /// random-access reads of specific regions.
            ///
            /// # Errors
            /// Returns [`Error::BoundsError`] if the region exceeds volume bounds.
            /// Returns [`Error::ModeMismatch`] if `T` does not match the file mode.
            pub fn subregion<T: Voxel>(
                &self,
                offset: [usize; 3],
                shape: [usize; 3],
            ) -> Result<VoxelBlock<T>, Error> {
                let bytes = self.vs_read_block_bytes(offset, shape)?;
                let data = self.vs_decode_block::<T>(&bytes)?;
                Ok(VoxelBlock {
                    offset,
                    shape,
                    data,
                })
            }

            /// Read the entire volume as a [`VoxelBlock<T>`].
            ///
            /// Shorthand for `subregion([0, 0, 0], [nx, ny, nz])`.
            ///
            /// # Errors
            /// Returns [`Error::ModeMismatch`] if `T` does not match the file mode.
            pub fn read_volume<T: Voxel>(&self) -> Result<VoxelBlock<T>, Error> {
                let s = self.shape();
                self.subregion([0, 0, 0], [s.nx, s.ny, s.nz])
            }

            /// Read the entire volume as `f32`, converting from any mode.
            ///
            /// Supports all real-valued modes (Int8, Int16, Uint16, Float32,
            /// Float16, Packed4Bit) and converts complex modes via magnitude.
            /// Returns the full volume in a single [`VoxelBlock<f32>`].
            pub fn read_volume_f32(&self) -> Result<VoxelBlock<f32>, Error> {
                let shape = self.shape();
                let offset = [0, 0, 0];
                let block_shape = [shape.nx, shape.ny, shape.nz];
                match self.mode() {
                    Mode::Float32 => self.subregion::<f32>(offset, block_shape),
                    Mode::Int16 => {
                        let block = self.subregion::<i16>(offset, block_shape)?;
                        let data = crate::engine::convert::convert_i16_slice_to_f32(&block.data);
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                    Mode::Uint16 => {
                        let block = self.subregion::<u16>(offset, block_shape)?;
                        let data = crate::engine::convert::convert_u16_slice_to_f32(&block.data);
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                    Mode::Int8 => {
                        let block = self.subregion::<i8>(offset, block_shape)?;
                        let data = crate::engine::convert::convert_i8_slice_to_f32(&block.data);
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                    #[cfg(feature = "f16")]
                    Mode::Float16 => {
                        let block = self.subregion::<crate::f16>(offset, block_shape)?;
                        let data = block.data.iter().map(|&v| f32::from(v)).collect();
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                    #[cfg(not(feature = "f16"))]
                    Mode::Float16 => Err(Error::UnsupportedMode),
                    Mode::Float32Complex => {
                        let block = self.subregion::<Float32Complex>(offset, block_shape)?;
                        let data = block
                            .data
                            .iter()
                            .map(|c| c.to_real(ComplexToRealStrategy::Magnitude))
                            .collect();
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                    Mode::Int16Complex => {
                        let block = self.subregion::<Int16Complex>(offset, block_shape)?;
                        let data = block
                            .data
                            .iter()
                            .map(|c| c.to_real(ComplexToRealStrategy::Magnitude))
                            .collect();
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                    Mode::Packed4Bit => {
                        let bytes = self.vs_read_block_bytes(offset, block_shape)?;
                        let unpacked = crate::engine::convert::unpack_u4_bytes_to_u8(
                            &bytes,
                            shape.nx,
                            shape.ny * shape.nz,
                        );
                        let data = unpacked.iter().map(|&v| v as f32).collect();
                        Ok(VoxelBlock {
                            offset,
                            shape: block_shape,
                            data,
                        })
                    }
                }
            }

            /// Read the entire volume as `u8`, unpacking from Mode 101 (Packed4Bit).
            ///
            /// Each `u8` value is in the range 0–15.
            ///
            /// # Errors
            /// Returns [`Error::ModeMismatch`] if the file mode is not [`Mode::Packed4Bit`].
            pub fn read_volume_u8(&self) -> Result<VoxelBlock<u8>, Error> {
                if self.mode() != Mode::Packed4Bit {
                    return Err(Error::ModeMismatch {
                        file_mode: self.mode(),
                        requested_mode: Mode::Packed4Bit,
                    });
                }
                let shape = self.shape();
                let block_shape = [shape.nx, shape.ny, shape.nz];
                let bytes = self.vs_read_block_bytes([0, 0, 0], block_shape)?;
                let data = crate::engine::convert::unpack_u4_bytes_to_u8(
                    &bytes,
                    shape.nx,
                    shape.ny * shape.nz,
                );
                Ok(VoxelBlock {
                    offset: [0, 0, 0],
                    shape: block_shape,
                    data,
                })
            }

            /// Iterate over Z-slices, converting each to `f32` automatically.
            ///
            /// Supports all real-valued MRC modes (Int8, Int16, Uint16, Float32,
            /// Float16, Packed4Bit) and converts complex modes via magnitude. This is the
            /// most convenient method for viewing or processing cryo-EM data.
            ///
            /// # Errors
            /// Returns [`Error::ModeMismatch`] if no matching conversion exists.
            pub fn slices_f32(&self) -> VoxelIter<'_, f32> {
                if self.mode() == Mode::Packed4Bit {
                    let shape = self.shape();
                    let nx = shape.nx;
                    let ny = shape.ny;
                    let nz = shape.nz;
                    return Box::new((0..nz).map(move |z| {
                        let bytes = self.vs_read_block_bytes([0, 0, z], [nx, ny, 1])?;
                        let data = crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, nx, ny)
                            .iter()
                            .map(|&v| v as f32)
                            .collect();
                        Ok(VoxelBlock {
                            offset: [0, 0, z],
                            shape: [nx, ny, 1],
                            data,
                        })
                    }));
                }
                iter_f32_helper(
                    self.mode(),
                    self.slices::<f32>(),
                    self.slices::<i16>(),
                    self.slices::<u16>(),
                    self.slices::<i8>(),
                    #[cfg(feature = "f16")]
                    self.slices::<crate::f16>(),
                    self.slices::<Float32Complex>(),
                    self.slices::<Int16Complex>(),
                )
            }
            /// Iterate over Z-slabs, converting each to `f32` automatically.
            ///
            /// See [`slices_f32`](Self::slices_f32) for supported mode conversions.
            pub fn slabs_f32(&self, k: usize) -> VoxelIter<'_, f32> {
                if self.mode() == Mode::Packed4Bit {
                    let volume_shape = self.shape();
                    let nx = volume_shape.nx;
                    let ny = volume_shape.ny;
                    let k = k.max(1);
                    let mut z = 0usize;
                    return Box::new(std::iter::from_fn(move || {
                        if z >= volume_shape.nz {
                            return None;
                        }
                        let start = z;
                        let sz = k.min(volume_shape.nz - z);
                        z += sz;
                        let bytes = match self.vs_read_block_bytes([0, 0, start], [nx, ny, sz]) {
                            Ok(b) => b,
                            Err(e) => return Some(Err(e)),
                        };
                        let data =
                            crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, nx, ny * sz)
                                .iter()
                                .map(|&v| v as f32)
                                .collect();
                        Some(Ok(VoxelBlock {
                            offset: [0, 0, start],
                            shape: [nx, ny, sz],
                            data,
                        }))
                    }));
                }
                iter_f32_helper(
                    self.mode(),
                    self.slabs::<f32>(k),
                    self.slabs::<i16>(k),
                    self.slabs::<u16>(k),
                    self.slabs::<i8>(k),
                    #[cfg(feature = "f16")]
                    self.slabs::<crate::f16>(k),
                    self.slabs::<Float32Complex>(k),
                    self.slabs::<Int16Complex>(k),
                )
            }
            /// Iterate over Z-slices as `u8`, narrowing from Mode 6 (Uint16)
            /// or unpacking from Mode 101 (Packed4Bit).
            ///
            /// For Uint16 files each 16-bit value is narrowed to 8 bits; values
            /// exceeding 255 produce an error.
            /// For Packed4Bit files each nibble is unpacked to `u8` (range 0–15).
            ///
            /// # Errors
            /// Returns [`Error::ModeMismatch`] if the file mode is not `Uint16` or `Packed4Bit`.
            pub fn slices_u8(&self) -> VoxelIter<'_, u8> {
                if self.mode() == Mode::Packed4Bit {
                    let shape = self.shape();
                    let nx = shape.nx;
                    let ny = shape.ny;
                    let nz = shape.nz;
                    return Box::new((0..nz).map(move |z| {
                        let bytes = self.vs_read_block_bytes([0, 0, z], [nx, ny, 1])?;
                        let data = crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, nx, ny);
                        Ok(VoxelBlock {
                            offset: [0, 0, z],
                            shape: [nx, ny, 1],
                            data,
                        })
                    }));
                }
                if self.mode() != Mode::Uint16 {
                    return Box::new(std::iter::once(Err(Error::ModeMismatch {
                        file_mode: self.mode(),
                        requested_mode: Mode::Uint16,
                    })));
                }
                Box::new(self.slices::<u16>().map(|b| {
                    let b = b?;
                    Ok(VoxelBlock {
                        offset: b.offset,
                        shape: b.shape,
                        data: crate::engine::convert::convert_u16_slice_to_u8(&b.data)?,
                    })
                }))
            }
            /// Iterate over Z-slabs as `u8`, narrowing from Mode 6 (Uint16)
            /// or unpacking from Mode 101 (Packed4Bit).
            ///
            /// See [`slices_u8`](Self::slices_u8) for mode-specific behaviour.
            pub fn slabs_u8(&self, k: usize) -> VoxelIter<'_, u8> {
                if self.mode() == Mode::Packed4Bit {
                    let volume_shape = self.shape();
                    let nx = volume_shape.nx;
                    let ny = volume_shape.ny;
                    let k = k.max(1);
                    let mut z = 0usize;
                    return Box::new(std::iter::from_fn(move || {
                        if z >= volume_shape.nz {
                            return None;
                        }
                        let start = z;
                        let sz = k.min(volume_shape.nz - z);
                        z += sz;
                        let bytes = match self.vs_read_block_bytes([0, 0, start], [nx, ny, sz]) {
                            Ok(b) => b,
                            Err(e) => return Some(Err(e)),
                        };
                        let data =
                            crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, nx, ny * sz);
                        Some(Ok(VoxelBlock {
                            offset: [0, 0, start],
                            shape: [nx, ny, sz],
                            data,
                        }))
                    }));
                }
                if self.mode() != Mode::Uint16 {
                    return Box::new(std::iter::once(Err(Error::ModeMismatch {
                        file_mode: self.mode(),
                        requested_mode: Mode::Uint16,
                    })));
                }
                let k = k.max(1);
                Box::new(self.slabs::<u16>(k).map(|b| {
                    let b = b?;
                    Ok(VoxelBlock {
                        offset: b.offset,
                        shape: b.shape,
                        data: crate::engine::convert::convert_u16_slice_to_u8(&b.data)?,
                    })
                }))
            }
            /// Iterate over Z-slices of a Mode 0 file, interpreting as signed or unsigned.
            ///
            /// Mode 0 (Int8) is ambiguous — some files store unsigned 8-bit data.
            /// Use this method to control interpretation via [`M0Interpretation`].
            ///
            /// # Errors
            /// Returns [`Error::ModeMismatch`] if the file mode is not `Int8`.
            pub fn slices_mode0(&self, interp: M0Interpretation) -> VoxelIter<'_, f32> {
                if self.mode() != Mode::Int8 {
                    return Box::new(std::iter::once(Err(Error::ModeMismatch {
                        file_mode: self.mode(),
                        requested_mode: Mode::Int8,
                    })));
                }
                let volume_shape = self.shape();
                Box::new((0..volume_shape.nz).map(move |z| {
                    let bytes =
                        self.vs_read_block_bytes([0, 0, z], [volume_shape.nx, volume_shape.ny, 1])?;
                    let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
                    Ok(VoxelBlock {
                        offset: [0, 0, z],
                        shape: [volume_shape.nx, volume_shape.ny, 1],
                        data,
                    })
                }))
            }
            /// Iterate over Z-slabs of a Mode 0 file, interpreting as signed or unsigned.
            ///
            /// Like [`slices_mode0`](Self::slices_mode0) but reads `k` slices per
            /// iteration for improved throughput.
            ///
            /// # Errors
            /// Returns [`Error::ModeMismatch`] if the file mode is not `Int8`.
            pub fn slabs_mode0(&self, k: usize, interp: M0Interpretation) -> VoxelIter<'_, f32> {
                if self.mode() != Mode::Int8 {
                    return Box::new(std::iter::once(Err(Error::ModeMismatch {
                        file_mode: self.mode(),
                        requested_mode: Mode::Int8,
                    })));
                }
                let volume_shape = self.shape();
                let k = k.max(1);
                let mut z = 0usize;
                Box::new(std::iter::from_fn(move || {
                    if z >= volume_shape.nz {
                        return None;
                    }
                    let start = z;
                    let sz = k.min(volume_shape.nz - z);
                    z += sz;
                    let bytes = match self
                        .vs_read_block_bytes([0, 0, start], [volume_shape.nx, volume_shape.ny, sz])
                    {
                        Ok(b) => b,
                        Err(e) => return Some(Err(e)),
                    };
                    let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
                    Some(Ok(VoxelBlock {
                        offset: [0, 0, start],
                        shape: [volume_shape.nx, volume_shape.ny, sz],
                        data,
                    }))
                }))
            }
        }
    };
}

impl_inherent_reader_methods!(crate::Reader);
#[cfg(feature = "mmap")]
impl_inherent_reader_methods!(crate::MmapReader);

/// Shared helper for `slices_f32` / `slabs_f32` — selects the correct
/// conversion based on file mode.
///
/// Supports all real-valued modes plus Float16, and converts complex modes
/// via magnitude (most common default for real-valued visualisation).
#[cfg(feature = "f16")]
#[allow(clippy::too_many_arguments)]
fn iter_f32_helper<'a, I, I16, I16E, U16, U16E, I8, I8E, IF16, IF16E, IC32, IC32E, IC16, IC16E>(
    mode: Mode,
    iter_f32: I,
    iter_i16: I16,
    iter_u16: U16,
    iter_i8: I8,
    iter_f16: IF16,
    iter_c32: IC32,
    iter_c16: IC16,
) -> VoxelIter<'a, f32>
where
    I: Iterator<Item = Result<VoxelBlock<f32>, Error>> + 'a,
    I16: Iterator<Item = Result<VoxelBlock<i16>, I16E>> + 'a,
    I16E: Into<Error>,
    U16: Iterator<Item = Result<VoxelBlock<u16>, U16E>> + 'a,
    U16E: Into<Error>,
    I8: Iterator<Item = Result<VoxelBlock<i8>, I8E>> + 'a,
    I8E: Into<Error>,
    IF16: Iterator<Item = Result<VoxelBlock<crate::f16>, IF16E>> + 'a,
    IF16E: Into<Error>,
    IC32: Iterator<Item = Result<VoxelBlock<Float32Complex>, IC32E>> + 'a,
    IC32E: Into<Error>,
    IC16: Iterator<Item = Result<VoxelBlock<Int16Complex>, IC16E>> + 'a,
    IC16E: Into<Error>,
{
    match mode {
        Mode::Float32 => Box::new(iter_f32) as Box<dyn Iterator<Item = _>>,
        Mode::Int16 => Box::new(iter_i16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_i16_slice_to_f32(&b.data),
            })
        })),
        Mode::Uint16 => Box::new(iter_u16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_u16_slice_to_f32(&b.data),
            })
        })),
        Mode::Int8 => Box::new(iter_i8.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_i8_slice_to_f32(&b.data),
            })
        })),
        #[cfg(feature = "f16")]
        Mode::Float16 => Box::new(iter_f16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: b.data.iter().map(|&v| f32::from(v)).collect(),
            })
        })),
        Mode::Float32Complex => Box::new(iter_c32.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: b
                    .data
                    .iter()
                    .map(|c| c.to_real(crate::mode::ComplexToRealStrategy::Magnitude))
                    .collect(),
            })
        })),
        Mode::Int16Complex => Box::new(iter_c16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: b
                    .data
                    .iter()
                    .map(|c| c.to_real(crate::mode::ComplexToRealStrategy::Magnitude))
                    .collect(),
            })
        })),
        _ => Box::new(std::iter::once(Err(Error::UnsupportedMode))),
    }
}

/// Version of [`iter_f32_helper`] used when the `f16` feature is disabled.
/// Omits the Float16 iterator parameter and returns `UnsupportedMode` for Float16 mode.
#[cfg(not(feature = "f16"))]
#[allow(clippy::too_many_arguments)]
fn iter_f32_helper<'a, I, I16, I16E, U16, U16E, I8, I8E, IC32, IC32E, IC16, IC16E>(
    mode: Mode,
    iter_f32: I,
    iter_i16: I16,
    iter_u16: U16,
    iter_i8: I8,
    iter_c32: IC32,
    iter_c16: IC16,
) -> VoxelIter<'a, f32>
where
    I: Iterator<Item = Result<VoxelBlock<f32>, Error>> + 'a,
    I16: Iterator<Item = Result<VoxelBlock<i16>, I16E>> + 'a,
    I16E: Into<Error>,
    U16: Iterator<Item = Result<VoxelBlock<u16>, U16E>> + 'a,
    U16E: Into<Error>,
    I8: Iterator<Item = Result<VoxelBlock<i8>, I8E>> + 'a,
    I8E: Into<Error>,
    IC32: Iterator<Item = Result<VoxelBlock<Float32Complex>, IC32E>> + 'a,
    IC32E: Into<Error>,
    IC16: Iterator<Item = Result<VoxelBlock<Int16Complex>, IC16E>> + 'a,
    IC16E: Into<Error>,
{
    match mode {
        Mode::Float32 => Box::new(iter_f32) as Box<dyn Iterator<Item = _>>,
        Mode::Int16 => Box::new(iter_i16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_i16_slice_to_f32(&b.data),
            })
        })),
        Mode::Uint16 => Box::new(iter_u16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_u16_slice_to_f32(&b.data),
            })
        })),
        Mode::Int8 => Box::new(iter_i8.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_i8_slice_to_f32(&b.data),
            })
        })),
        Mode::Float16 => Box::new(std::iter::once(Err(Error::UnsupportedMode))),
        Mode::Float32Complex => Box::new(iter_c32.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: b
                    .data
                    .iter()
                    .map(|c| c.to_real(crate::mode::ComplexToRealStrategy::Magnitude))
                    .collect(),
            })
        })),
        Mode::Int16Complex => Box::new(iter_c16.map(|b| {
            let b = b.map_err(Into::into)?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: b
                    .data
                    .iter()
                    .map(|c| c.to_real(crate::mode::ComplexToRealStrategy::Magnitude))
                    .collect(),
            })
        })),
        _ => Box::new(std::iter::once(Err(Error::UnsupportedMode))),
    }
}

/// Validate a block read/write request.
///
/// Checks that the requested block is fully contained within the volume bounds
/// and that the data region is large enough for the last row of the block.
/// Returns the total byte length of the gathered block.
///
/// # Errors
///
/// * [`Error::BoundsError`] if the block exceeds volume bounds or the data length.
pub(crate) fn validate_block_bounds(
    volume_shape: VolumeShape,
    mode: Mode,
    data_len: usize,
    offset: [usize; 3],
    block_shape: [usize; 3],
) -> Result<usize, Error> {
    let [nx, ny, nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
    let [ox, oy, oz] = offset;
    let [sx, sy, sz] = block_shape;

    // Use checked arithmetic to avoid wrap-around on maliciously large offsets.
    if ox.checked_add(sx).is_none_or(|end| end > nx)
        || oy.checked_add(sy).is_none_or(|end| end > ny)
        || oz.checked_add(sz).is_none_or(|end| end > nz)
    {
        return Err(Error::BoundsError);
    }

    let count = sx
        .checked_mul(sy)
        .and_then(|v| v.checked_mul(sz))
        .ok_or(Error::BoundsError)?;
    let block_row_bytes = sx.div_ceil(2);
    let byte_len = if mode == Mode::Packed4Bit {
        block_row_bytes
            .checked_mul(sy)
            .and_then(|v| v.checked_mul(sz))
            .ok_or(Error::BoundsError)?
    } else {
        mode.byte_size_for_count(count)
    };

    if count == 0 {
        return Ok(0);
    }

    // Verify the data region is large enough for the last row of the block.
    if mode == Mode::Packed4Bit {
        // Only byte-aligned X-offsets are supported (ox even) to avoid
        // nibble-level read-modify-write in gather/write paths.
        if ox % 2 != 0 {
            return Err(Error::BoundsError);
        }
        let vol_row_bytes = nx.div_ceil(2);
        let start_byte_in_row = ox / 2;
        let last_vol_row = (oz + sz - 1) * ny + (oy + sy - 1);
        let last_byte = last_vol_row
            .checked_mul(vol_row_bytes)
            .and_then(|b| b.checked_add(start_byte_in_row + block_row_bytes))
            .ok_or(Error::BoundsError)?;
        if last_byte > data_len {
            return Err(Error::BoundsError);
        }
    } else {
        let last_row_start = volume_shape
            .checked_linear_index([ox, oy + sy - 1, oz + sz - 1])
            .ok_or(Error::BoundsError)?;
        let last_byte = last_row_start
            .checked_add(sx)
            .map(|end| mode.byte_size_for_count(end))
            .ok_or(Error::BoundsError)?;
        if last_byte > data_len {
            return Err(Error::BoundsError);
        }
    }

    Ok(byte_len)
}

/// Gather a non-contiguous 3D block from raw data bytes into a contiguous Vec.
///
/// The source `data` is treated as a C-ordered `[nx, ny, nz]` array where X is the
/// fastest axis. The returned Vec contains the sub-block in C-order.
///
/// For [`Mode::Packed4Bit`], the byte layout uses 2 voxels per byte; voxel index
/// `v` maps to byte `v / 2` in the data array.
pub(crate) fn gather_block_bytes(
    data: &[u8],
    volume_shape: VolumeShape,
    mode: Mode,
    offset: [usize; 3],
    block_shape: [usize; 3],
) -> Vec<u8> {
    let [nx, ny, _nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
    let [ox, oy, oz] = offset;
    let [sx, sy, sz] = block_shape;

    if mode == Mode::Packed4Bit {
        // Each byte holds two voxels.  Each row has sx.div_ceil(2) packed bytes.
        let vol_row_bytes = nx.div_ceil(2);
        let block_row_bytes = sx.div_ceil(2);
        let byte_len = block_row_bytes * sy * sz;
        let mut dst = vec![0u8; byte_len];

        // Fast path: full XY slab is contiguous in the file.
        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let slice_bytes = ny * vol_row_bytes;
            let start = oz * slice_bytes;
            let byte_len = sz * slice_bytes;
            return data[start..start + byte_len].to_vec();
        }

        for z in 0..sz {
            for y in 0..sy {
                let vol_row = (oz + z) * ny + (oy + y);
                let src_start = vol_row * vol_row_bytes + ox / 2;
                let dst_start = (y + z * sy) * block_row_bytes;
                dst[dst_start..dst_start + block_row_bytes]
                    .copy_from_slice(&data[src_start..src_start + block_row_bytes]);
            }
        }
        return dst;
    }

    let b = mode.byte_size();
    let voxel_count = sx * sy * sz;
    let byte_len = voxel_count * b;
    let mut dst = vec![0u8; byte_len];

    // Fast path: full XY slab is contiguous in the file.
    if ox == 0 && sx == nx && oy == 0 && sy == ny {
        let linear = oz * nx * ny;
        let start = linear * b;
        let byte_len = sx * sy * sz * b;
        return data[start..start + byte_len].to_vec();
    }

    for z in 0..sz {
        for y in 0..sy {
            let src_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
            let src_start = src_linear * b;
            let dst_linear = y * sx + z * sx * sy;
            let dst_start = dst_linear * b;
            dst[dst_start..dst_start + sx * b]
                .copy_from_slice(&data[src_start..src_start + sx * b]);
        }
    }
    dst
}

/// Encode a typed voxel block into a mutable byte buffer (the full data region).
///
/// Handles both contiguous (full XY slab) and scattered (row-by-row) write paths.
/// This is the complementary write-side operation to [`gather_block_bytes`].
///
/// # Errors
/// Returns `Error::BoundsError` if the block exceeds the buffer boundaries.
/// Returns `Error::TypeMismatch` if the byte count is misaligned.
pub(crate) fn encode_block_to_buf<T: EndianCodec + Sync>(
    block: &VoxelBlock<T>,
    volume_shape: VolumeShape,
    bytes_per_voxel: usize,
    file_endian: FileEndian,
    data_offset: usize,
    buf: &mut [u8],
) -> Result<(), Error> {
    let [nx, ny, _nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
    let [ox, oy, oz] = block.offset;
    let [sx, sy, sz] = block.shape;
    let b = bytes_per_voxel;

    // Fast path: full XY slab is contiguous in the buffer.
    if ox == 0 && sx == nx && oy == 0 && sy == ny {
        let linear = oz * nx * ny;
        let start_byte = data_offset + linear * b;
        let byte_len = sx * sy * sz * b;
        let end_byte = start_byte + byte_len;
        if end_byte > buf.len() {
            return Err(Error::BoundsError);
        }
        encode_slice(&block.data, &mut buf[start_byte..end_byte], file_endian)?;
        return Ok(());
    }

    // Scatter path: write row by row.
    for z in 0..sz {
        for y in 0..sy {
            let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
            let file_start = data_offset + file_linear * b;
            let block_idx = y * sx + z * sx * sy;
            if block_idx + sx > block.data.len() {
                return Err(Error::BoundsError);
            }
            let row_values = &block.data[block_idx..block_idx + sx];
            let row_end = file_start + sx * b;
            if row_end > buf.len() {
                return Err(Error::BoundsError);
            }
            encode_slice(row_values, &mut buf[file_start..row_end], file_endian)?;
        }
    }
    Ok(())
}

/// Write already-packed bytes into the data buffer at a given offset and shape.
///
/// This is the byte-level version of [`encode_block_to_buf`], used for Mode 101
/// (Packed4Bit) writes where the data is already packed row-by-row.
/// Endianness does not apply (nibble ordering is endian-independent).
///
/// Note: only supports full-row writes (`block_offset[0] == 0`). Sub-XY blocks
/// with non-zero X-offset would need read-modify-write at the nibble level.
pub(crate) fn write_block_bytes(
    packed: &[u8],
    volume_shape: VolumeShape,
    block_offset: [usize; 3],
    block_shape: [usize; 3],
    data_offset: usize,
    buf: &mut [u8],
) -> Result<(), Error> {
    let [nx, ny, _nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
    let [ox, oy, oz] = block_offset;
    let [sx, sy, sz] = block_shape;
    let file_row_bytes = nx.div_ceil(2); // packed bytes per row in the volume
    let block_row_bytes = sx.div_ceil(2); // packed bytes per row in the block

    // Only full-row (ox=0) blocks are supported to avoid nibble-level RMW.
    assert!(ox == 0, "write_block_bytes requires ox == 0");

    // Fast path: full XY slab is contiguous in the buffer.
    if sx == nx && oy == 0 && sy == ny {
        let slice_bytes = ny * file_row_bytes;
        let start_byte = data_offset + oz * slice_bytes;
        let byte_len = sz * slice_bytes;
        let end_byte = start_byte + byte_len;
        if end_byte > buf.len() {
            return Err(Error::BoundsError);
        }
        buf[start_byte..end_byte].copy_from_slice(&packed[..byte_len]);
        return Ok(());
    }

    // Scatter path: write row by row.
    // Each row in the volume occupies file_row_bytes; each row in the block
    // occupies block_row_bytes.
    for z in 0..sz {
        for y in 0..sy {
            let vol_row = (oz + z) * ny + (oy + y);
            let file_start = data_offset + vol_row * file_row_bytes;
            let file_end = file_start + block_row_bytes;
            if file_end > buf.len() {
                return Err(Error::BoundsError);
            }
            let packed_start = (y + z * sy) * block_row_bytes;
            let packed_end = packed_start + block_row_bytes;
            if packed_end > packed.len() {
                return Err(Error::BoundsError);
            }
            buf[file_start..file_end].copy_from_slice(&packed[packed_start..packed_end]);
        }
    }
    Ok(())
}

/// Decode a raw byte block to the requested voxel type.
///
/// Performs endian conversion if the file endianness differs from the host.
/// Returns [`Error::ModeMismatch`] if `T` does not match `file_mode`.
pub(crate) fn decode_block<T: Voxel>(
    bytes: &[u8],
    file_mode: Mode,
    endian: FileEndian,
) -> Result<Vec<T>, Error> {
    if T::MODE != file_mode {
        return Err(Error::ModeMismatch {
            file_mode,
            requested_mode: T::MODE,
        });
    }

    if endian == FileEndian::native() {
        decode_native_endian(bytes)
    } else {
        decode_slice(bytes, endian)
    }
}

/// Fast native-endian decode: copy bytes directly into a `Vec<T>`.
///
/// This is a thin `memcpy` wrapper used when the file endianness matches the
/// host, avoiding per-element swapping.
///
/// # Safety
/// This function uses `unsafe` to copy raw bytes into a typed `Vec`. The caller
/// must ensure that `bytes.len()` is an exact multiple of `T::BYTE_SIZE` and
/// that the byte pattern is valid for `T`. For MRC data this always holds
/// because the byte count is derived from `mode.byte_size() * count`.
pub(crate) fn decode_native_endian<T: EndianCodec + Copy>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    let n = bytes.len() / T::BYTE_SIZE;
    debug_assert_eq!(
        bytes.len() % T::BYTE_SIZE,
        0,
        "decode_native_endian: bytes.len() ({}) must be a multiple of T::BYTE_SIZE ({})",
        bytes.len(),
        T::BYTE_SIZE
    );
    let mut result = Vec::with_capacity(n);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), result.as_mut_ptr() as *mut u8, bytes.len());
        result.set_len(n);
    }
    Ok(result)
}

/// Parse and validate an MRC header from raw bytes.
///
/// Returns the decoded header, any warning messages, the detected endianness,
/// and the expected data size in bytes.
pub(crate) fn parse_header(
    header_bytes: &[u8; 1024],
    permissive: bool,
) -> Result<(crate::Header, Vec<String>, crate::FileEndian, usize), crate::Error> {
    let (header, endian_warning) = crate::Header::decode_from_bytes_with_info(header_bytes);
    let mut warnings = if permissive {
        header
            .validate_permissive()
            .map_err(crate::Error::InvalidHeaderDetailed)?
    } else {
        header
            .validate_detailed()
            .map_err(crate::Error::InvalidHeaderDetailed)?;
        Vec::new()
    };
    if let Some(msg) = endian_warning {
        warnings.push(msg.to_string());
    }
    let data_size = header.data_size().ok_or(crate::Error::InvalidHeader)?;
    let endian = header.detect_endian();
    Ok((header, warnings, endian, data_size))
}

/// Default maximum decompressed bytes for compressed MRC files (256 GiB).
///
/// This is an absolute cap applied **before** parsing the header, preventing
/// decompression bombs where a small compressed file claims huge dimensions.
///
/// Used by [`Reader::open_gzip`](crate::Reader::open_gzip) and
/// [`Reader::open_bzip2`](crate::Reader::open_bzip2). Pass a custom value to
/// [`Reader::open_gzip_with_limit`](crate::Reader::open_gzip_with_limit) or
/// [`Reader::open_bzip2_with_limit`](crate::Reader::open_bzip2_with_limit) to
/// override.
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024 * 1024;

/// Components of a decompressed MRC file, returned by [`open_compressed`].
pub(crate) struct DecompressedMrc {
    /// Parsed MRC header.
    pub header: crate::Header,
    /// Extended header bytes.
    pub ext_header: Vec<u8>,
    /// Voxel data bytes.
    pub data: Vec<u8>,
    /// Non-fatal warnings (empty unless `permissive` was `true`).
    pub warnings: Vec<String>,
    /// Detected file endianness.
    pub endian: crate::FileEndian,
    /// Volume dimensions.
    pub shape: VolumeShape,
}

/// Open a compressed MRC file (gzip or bzip2) from a decoder.
///
/// Reads the decompressed stream into memory with a safety cap on total
/// output bytes (`max_bytes`). This cap is applied **before** parsing the
/// header, so a malicious file that claims huge dimensions cannot trigger
/// unbounded memory allocation.
///
/// After decompression, the header is parsed and the actual size is validated
/// against the header-declared size (unless in permissive mode).
///
/// # Safety limit
///
/// If the decompressed stream exceeds `max_bytes`, the function returns an
/// [`Error::Io`] with `"Decompressed data exceeds safety limit"`. The default
/// value is [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB), accessible through
/// [`Reader::open_gzip_with_limit`](crate::Reader::open_gzip_with_limit) and
/// [`Reader::open_bzip2_with_limit`](crate::Reader::open_bzip2_with_limit).
pub(crate) fn open_compressed<D: std::io::Read>(
    mut decoder: D,
    permissive: bool,
    max_bytes: u64,
) -> Result<DecompressedMrc, crate::Error> {
    // Read up to max_bytes + 1 so we can detect truncation by the cap.
    // If the stream exceeds max_bytes we return an error immediately,
    // before the header is ever inspected.
    let limit = max_bytes + 1;
    let mut buf = Vec::with_capacity(limit.min(1024 * 1024) as usize);
    decoder.by_ref().take(limit).read_to_end(&mut buf)?;

    if buf.len() > max_bytes as usize {
        return Err(crate::Error::Io(std::io::Error::other(format!(
            "Decompressed data exceeds safety limit of {max_bytes} bytes \
             ({} GiB). Refusing to allocate. \
             Use Reader::open_gzip_with_limit() or Reader::open_bzip2_with_limit() \
             with a larger max_bytes if you trust this file.",
            max_bytes / (1024 * 1024 * 1024),
        ))));
    }

    if buf.len() < 1024 {
        return Err(crate::Error::InvalidHeader);
    }

    let mut header_bytes = [0u8; 1024];
    header_bytes.copy_from_slice(&buf[..1024]);
    let (header, mut warnings, endian, data_size) = parse_header(&header_bytes, permissive)?;

    let ext_size = header.nsymbt as usize;

    if !permissive {
        if buf.len() != 1024 + ext_size + data_size {
            return Err(crate::Error::FileSizeMismatch {
                expected: 1024 + ext_size + data_size,
                actual: buf.len(),
            });
        }
    } else if buf.len() != 1024 + ext_size + data_size {
        warnings.push(format!(
            "File size mismatch: expected {} bytes, got {}",
            1024 + ext_size + data_size,
            buf.len()
        ));
    }

    // Clamp ext_header and data slices to available bytes (permissive mode
    // may reach here with a mismatched file, and slices must not panic).
    let ext_end = (1024 + ext_size).min(buf.len());
    let ext_header = buf[1024..ext_end].to_vec();
    let data = if ext_end < buf.len() {
        buf[ext_end..].to_vec()
    } else {
        Vec::new()
    };
    let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

    Ok(DecompressedMrc {
        header,
        ext_header,
        data,
        warnings,
        endian,
        shape,
    })
}

// ============================================================================
// ReaderCore implementations
// ============================================================================

impl private::Sealed for crate::Reader {}
impl VoxelSource for crate::Reader {
    fn vs_read_block_bytes<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Cow<'a, [u8]>, Error> {
        self.read_block_bytes(offset, shape).map(Cow::Owned)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.decode_block(bytes)
    }
}
impl ReaderCore for crate::Reader {
    fn shape(&self) -> VolumeShape {
        self.shape()
    }
    fn mode(&self) -> Mode {
        self.mode()
    }
    fn endian(&self) -> FileEndian {
        self.endian
    }
    fn header(&self) -> &Header {
        &self.header
    }
    fn ext_header_bytes(&self) -> &[u8] {
        &self.ext_header
    }
}
#[cfg(feature = "mmap")]
impl private::Sealed for crate::MmapReader {}
#[cfg(feature = "mmap")]
impl VoxelSource for crate::MmapReader {
    fn vs_read_block_bytes<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Cow<'a, [u8]>, Error> {
        // MmapReader has a zero-copy fast path for contiguous XY slabs.
        self.read_block_bytes_cow(offset, shape)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.decode_block(bytes)
    }
}
#[cfg(feature = "mmap")]
impl ReaderCore for crate::MmapReader {
    fn shape(&self) -> VolumeShape {
        self.shape()
    }
    fn mode(&self) -> Mode {
        self.mode()
    }
    fn endian(&self) -> FileEndian {
        self.endian()
    }
    fn header(&self) -> &Header {
        self.header()
    }
    fn ext_header_bytes(&self) -> &[u8] {
        self.ext_header_bytes()
    }
}
