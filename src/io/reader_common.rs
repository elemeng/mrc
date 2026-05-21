//! Shared helpers for all MRC reader implementations.
//!
//! This module contains the [`VoxelSource`] trait and helper functions that are
//! used by [`Reader`](crate::Reader), [`MmapReader`](crate::MmapReader),
//! [`GzipReader`](crate::GzipReader), [`Bzip2Reader`](crate::Bzip2Reader), and
//! [`MrcReader`](crate::MrcReader) to implement block validation, endian
//! decoding, and the `slices_f32` / `slabs_f32` convenience iterators.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::{EndianCodec, decode_slice};
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Mode};
use std::borrow::Cow;

mod private {
    /// Sealed trait marker — prevents external implementations of [`VoxelSource`].
    pub trait Sealed {}
}

/// Sealed trait unifying all reader types for generic iterators.
///
/// [`SliceIter`](crate::SliceIter), [`SlabIter`](crate::SlabIter), and
/// [`BlockIter`](crate::BlockIter) are generic over `VoxelSource` so they can
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
pub fn validate_block_bounds(
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
pub fn gather_block_bytes(
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

/// Decode a raw byte block to the requested voxel type.
///
/// Performs endian conversion if the file endianness differs from the host.
/// Returns [`Error::ModeMismatch`] if `T` does not match `file_mode`.
pub fn decode_block<T: Voxel>(
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
        Ok(decode_slice(bytes, endian))
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
pub fn decode_native_endian<T: EndianCodec + Copy>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    let n = bytes.len() / T::BYTE_SIZE;
    let mut result = Vec::with_capacity(n);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), result.as_mut_ptr() as *mut u8, bytes.len());
        result.set_len(n);
    }
    Ok(result)
}

/// Build a slice iterator that automatically converts common modes to `f32`.
///
/// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
/// Returns [`Error::UnsupportedMode`] for other modes.
///
/// The `read_bytes` callback abstracts over the concrete reader backend so the
/// same logic can be reused by [`Reader`](crate::Reader), [`MmapReader`](crate::MmapReader),
/// and the compression wrappers.
pub fn slices_f32<'a>(
    shape: VolumeShape,
    mode: Mode,
    endian: FileEndian,
    mut read_bytes: impl FnMut([usize; 3], [usize; 3]) -> Result<Cow<'a, [u8]>, Error> + 'a,
) -> Result<crate::SliceIterF32<'a>, Error> {
    let nx = shape.nx;
    let ny = shape.ny;
    let nz = shape.nz;

    Ok(Box::new((0..nz).map(move |z| {
        let bytes = read_bytes([0, 0, z], [nx, ny, 1])?;
        let data = match mode {
            Mode::Float32 => decode_block::<f32>(&bytes, mode, endian)?,
            Mode::Int16 => {
                let tmp = decode_block::<i16>(&bytes, mode, endian)?;
                crate::engine::convert::convert_i16_slice_to_f32(&tmp)
            }
            Mode::Uint16 => {
                let tmp = decode_block::<u16>(&bytes, mode, endian)?;
                crate::engine::convert::convert_u16_slice_to_f32(&tmp)
            }
            Mode::Int8 => {
                let tmp = decode_block::<i8>(&bytes, mode, endian)?;
                crate::engine::convert::convert_i8_slice_to_f32(&tmp)
            }
            _ => return Err(Error::UnsupportedMode),
        };
        Ok(VoxelBlock {
            offset: [0, 0, z],
            shape: [nx, ny, 1],
            data,
        })
    })))
}

/// Build a slab iterator that automatically converts common modes to `f32`.
///
/// A slab is a contiguous group of `k` Z-slices. Supported source modes and
/// error behaviour are the same as for [`slices_f32`].
pub fn slabs_f32<'a>(
    shape: VolumeShape,
    mode: Mode,
    endian: FileEndian,
    k: usize,
    mut read_bytes: impl FnMut([usize; 3], [usize; 3]) -> Result<Cow<'a, [u8]>, Error> + 'a,
) -> Result<crate::SliceIterF32<'a>, Error> {
    let nx = shape.nx;
    let ny = shape.ny;
    let nz = shape.nz;
    let k = k.max(1);

    Ok(Box::new((0..nz).step_by(k).map(move |z| {
        let sz = k.min(nz - z);
        let bytes = read_bytes([0, 0, z], [nx, ny, sz])?;
        let data = match mode {
            Mode::Float32 => decode_block::<f32>(&bytes, mode, endian)?,
            Mode::Int16 => {
                let tmp = decode_block::<i16>(&bytes, mode, endian)?;
                crate::engine::convert::convert_i16_slice_to_f32(&tmp)
            }
            Mode::Uint16 => {
                let tmp = decode_block::<u16>(&bytes, mode, endian)?;
                crate::engine::convert::convert_u16_slice_to_f32(&tmp)
            }
            Mode::Int8 => {
                let tmp = decode_block::<i8>(&bytes, mode, endian)?;
                crate::engine::convert::convert_i8_slice_to_f32(&tmp)
            }
            _ => return Err(Error::UnsupportedMode),
        };
        Ok(VoxelBlock {
            offset: [0, 0, z],
            shape: [nx, ny, sz],
            data,
        })
    })))
}

/// Parse and validate an MRC header from raw bytes.
///
/// Returns the decoded header, any warning messages, the detected endianness,
/// and the expected data size in bytes.
pub fn parse_header(
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

// ---------------------------------------------------------------------------
// Shared convenience iterator builders
// ---------------------------------------------------------------------------

/// Build a `slices_u8` iterator.
pub fn slices_u8<'a>(
    shape: VolumeShape,
    mode: Mode,
    mut read_bytes: impl FnMut([usize; 3], [usize; 3]) -> Result<Vec<u8>, Error> + 'a,
    mut decode_u16: impl FnMut(&[u8]) -> Result<Vec<u16>, Error> + 'a,
) -> Result<Box<dyn Iterator<Item = Result<VoxelBlock<u8>, Error>> + 'a>, Error> {
    if mode != Mode::Uint16 {
        return Err(Error::ModeMismatch {
            file_mode: mode,
            requested_mode: Mode::Uint16,
        });
    }
    let nx = shape.nx;
    let ny = shape.ny;
    let nz = shape.nz;
    Ok(Box::new((0..nz).map(move |z| {
        let bytes = read_bytes([0, 0, z], [nx, ny, 1])?;
        let u16_data = decode_u16(&bytes)?;
        let u8_data = crate::engine::convert::convert_u16_slice_to_u8(&u16_data)?;
        Ok(VoxelBlock {
            offset: [0, 0, z],
            shape: [nx, ny, 1],
            data: u8_data,
        })
    })))
}

/// Build a `slices_mode0` iterator.
pub fn slices_mode0<'a>(
    shape: VolumeShape,
    mode: Mode,
    interp: crate::mode::M0Interpretation,
    mut read_bytes: impl FnMut([usize; 3], [usize; 3]) -> Result<Vec<u8>, Error> + 'a,
) -> Box<dyn Iterator<Item = Result<VoxelBlock<f32>, Error>> + 'a> {
    let nx = shape.nx;
    let ny = shape.ny;
    let nz = shape.nz;
    Box::new((0..nz).map(move |z| {
        if mode != Mode::Int8 {
            return Err(Error::ModeMismatch {
                file_mode: mode,
                requested_mode: Mode::Int8,
            });
        }
        let bytes = read_bytes([0, 0, z], [nx, ny, 1])?;
        let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
        Ok(VoxelBlock {
            offset: [0, 0, z],
            shape: [nx, ny, 1],
            data,
        })
    }))
}

/// Build a `slabs_mode0` iterator.
pub fn slabs_mode0<'a>(
    shape: VolumeShape,
    mode: Mode,
    k: usize,
    interp: crate::mode::M0Interpretation,
    mut read_bytes: impl FnMut([usize; 3], [usize; 3]) -> Result<Vec<u8>, Error> + 'a,
) -> Box<dyn Iterator<Item = Result<VoxelBlock<f32>, Error>> + 'a> {
    let nx = shape.nx;
    let ny = shape.ny;
    let nz = shape.nz;
    let mut z = 0usize;
    let mut error_returned = false;
    Box::new(std::iter::from_fn(move || {
        if error_returned {
            return None;
        }
        if mode != Mode::Int8 {
            error_returned = true;
            return Some(Err(Error::ModeMismatch {
                file_mode: mode,
                requested_mode: Mode::Int8,
            }));
        }
        if z >= nz {
            return None;
        }
        let start = z;
        let size = k.min(nz - z);
        z += size;
        let bytes = match read_bytes([0, 0, start], [nx, ny, size]) {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };
        let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
        Some(Ok(VoxelBlock {
            offset: [0, 0, start],
            shape: [nx, ny, size],
            data,
        }))
    }))
}

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

#[cfg(feature = "gzip")]
impl private::Sealed for crate::GzipReader {}
#[cfg(feature = "gzip")]
impl VoxelSource for crate::GzipReader {
    fn vs_read_block_bytes<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Cow<'a, [u8]>, Error> {
        self.0.read_block_bytes(offset, shape).map(Cow::Owned)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.0.decode_block(bytes)
    }
}

#[cfg(feature = "bzip2")]
impl private::Sealed for crate::Bzip2Reader {}
#[cfg(feature = "bzip2")]
impl VoxelSource for crate::Bzip2Reader {
    fn vs_read_block_bytes<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Cow<'a, [u8]>, Error> {
        self.0.read_block_bytes(offset, shape).map(Cow::Owned)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.0.decode_block(bytes)
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
        self.read_block_bytes(offset, shape).map(Cow::Owned)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.decode_block(bytes)
    }
}

impl private::Sealed for crate::MrcReader {}
impl VoxelSource for crate::MrcReader {
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
