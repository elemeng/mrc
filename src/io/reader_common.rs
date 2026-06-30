//! Shared helpers for all MRC reader implementations.
//!
//! This module contains the [`VoxelSource`] trait and helper functions that are
//! used by [`Reader`](crate::Reader) and [`MmapReader`](crate::MmapReader) to implement block validation, endian
//! decoding, and the `slices_f32` / `slabs_f32` convenience iterators.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::{EndianCodec, decode_slice, encode_slice};
use crate::engine::endian::FileEndian;
use crate::iter::{RegionIter, SlabStepper, SliceStepper, TileStepper};
use crate::mode::{Float32Complex, Int16Complex, M0Interpretation, Voxel};
use crate::{Error, Header, Mode};
use std::borrow::Cow;

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
            pub fn slices<T: Voxel>(&self) -> RegionIter<'_, T, $ty, SliceStepper> {
                RegionIter::with_stepper(self, self.shape(), SliceStepper::new())
            }
            pub fn slabs<T: Voxel>(&self, k: usize) -> RegionIter<'_, T, $ty, SlabStepper> {
                RegionIter::with_stepper(self, self.shape(), SlabStepper::new(k))
            }
            pub fn tiles<T: Voxel>(&self, tile_shape: [usize; 3]) -> RegionIter<'_, T, $ty, TileStepper> {
                RegionIter::with_stepper(self, self.shape(), TileStepper::new(tile_shape))
            }
            pub fn images<T: Voxel>(&self) -> RegionIter<'_, T, $ty, SliceStepper> {
                self.slices()
            }
            pub fn image_stack<T: Voxel>(&self, k: usize) -> RegionIter<'_, T, $ty, SlabStepper> {
                self.slabs(k)
            }
            pub fn planes<T: Voxel>(&self) -> RegionIter<'_, T, $ty, SliceStepper> {
                self.slices()
            }
            pub fn plane_stack<T: Voxel>(&self, k: usize) -> RegionIter<'_, T, $ty, SlabStepper> {
                self.slabs(k)
            }
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
            pub fn subregion<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error> {
                let bytes = self.vs_read_block_bytes(offset, shape)?;
                let data = self.vs_decode_block::<T>(&bytes)?;
                Ok(VoxelBlock { offset, shape, data })
            }

            pub fn slices_f32(&self) -> VoxelIter<'_, f32> {
                iter_f32_helper(
                    self.mode(),
                    self.slices::<f32>(),
                    self.slices::<i16>(),
                    self.slices::<u16>(),
                    self.slices::<i8>(),
                    self.slices::<crate::f16>(),
                    self.slices::<Float32Complex>(),
                    self.slices::<Int16Complex>(),
                )
            }
            pub fn slabs_f32(&self, k: usize) -> VoxelIter<'_, f32> {
                iter_f32_helper(
                    self.mode(),
                    self.slabs::<f32>(k),
                    self.slabs::<i16>(k),
                    self.slabs::<u16>(k),
                    self.slabs::<i8>(k),
                    self.slabs::<crate::f16>(k),
                    self.slabs::<Float32Complex>(k),
                    self.slabs::<Int16Complex>(k),
                )
            }
            pub fn slices_u8(&self) -> VoxelIter<'_, u8> {
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
            pub fn slices_mode0(&self, interp: M0Interpretation) -> VoxelIter<'_, f32> {
                if self.mode() != Mode::Int8 {
                    return Box::new(std::iter::once(Err(Error::ModeMismatch {
                        file_mode: self.mode(),
                        requested_mode: Mode::Int8,
                    })));
                }
                let volume_shape = self.shape();
                Box::new((0..volume_shape.nz).map(move |z| {
                    let bytes = self.vs_read_block_bytes([0, 0, z], [volume_shape.nx, volume_shape.ny, 1])?;
                    let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
                    Ok(VoxelBlock {
                        offset: [0, 0, z],
                        shape: [volume_shape.nx, volume_shape.ny, 1],
                        data,
                    })
                }))
            }
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
                    if z >= volume_shape.nz { return None; }
                    let start = z;
                    let sz = k.min(volume_shape.nz - z);
                    z += sz;
                    let bytes = match self.vs_read_block_bytes([0, 0, start], [volume_shape.nx, volume_shape.ny, sz]) {
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
        #[cfg(not(feature = "f16"))]
        Mode::Float16 => {
            let _ = (iter_f32, iter_i16, iter_u16, iter_i8, iter_f16, iter_c32, iter_c16);
            Box::new(std::iter::once(Err(Error::UnsupportedMode)))
        }
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
/// * [`Error::UnsupportedMode`] if the mode is [`Mode::Packed4Bit`].
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

    if ox + sx > nx || oy + sy > ny || oz + sz > nz {
        return Err(Error::BoundsError);
    }

    if mode == Mode::Packed4Bit {
        return Err(Error::UnsupportedMode);
    }

    let count = sx
        .checked_mul(sy)
        .and_then(|v| v.checked_mul(sz))
        .ok_or(Error::BoundsError)?;
    let byte_len = mode.byte_size_for_count(count);

    if count == 0 {
        return Ok(0);
    }

    // Verify the data region is large enough for the last row of the block.
    let last_row_start = volume_shape
        .checked_linear_index([ox, oy + sy - 1, oz + sz - 1])
        .ok_or(Error::BoundsError)?;
    let last_byte = last_row_start
        .checked_mul(mode.byte_size())
        .and_then(|s| s.checked_add(sx * mode.byte_size()))
        .ok_or(Error::BoundsError)?;

    if last_byte > data_len {
        return Err(Error::BoundsError);
    }

    Ok(byte_len)
}

/// Gather a non-contiguous 3D block from raw data bytes into a contiguous Vec.
///
/// The source `data` is treated as a C-ordered `[nx, ny, nz]` array where X is the
/// fastest axis. The returned Vec contains the sub-block in C-order.
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
    let b = mode.byte_size();

    // Fast path: full XY slab is contiguous in the file.
    if ox == 0 && sx == nx && oy == 0 && sy == ny {
        let linear = oz * nx * ny;
        let start = linear * b;
        let byte_len = sx * sy * sz * b;
        return data[start..start + byte_len].to_vec();
    }

    let mut dst = vec![0u8; sx * sy * sz * b];
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
/// Reads the entire decompressed stream into memory, parses the header,
/// validates size, and returns the components needed to construct a [`Reader`].
pub(crate) fn open_compressed<D: std::io::Read>(
    mut decoder: D,
    permissive: bool,
) -> Result<DecompressedMrc, crate::Error> {
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;

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

    let ext_header = buf[1024..1024 + ext_size].to_vec();
    let data = buf[1024 + ext_size..].to_vec();
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
