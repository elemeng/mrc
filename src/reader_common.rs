//! Shared helpers for all MRC reader implementations.

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

/// Internal trait unifying all reader types for generic iterators.
///
/// This trait is sealed: it can only be implemented by types inside this crate.
#[doc(hidden)]
pub trait VoxelSource: private::Sealed {
    fn vs_read_block_bytes<'a>(&'a self, offset: [usize; 3], shape: [usize; 3]) -> Result<Cow<'a, [u8]>, Error>;
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error>;
}

/// Validate a block read request and compute byte range.
///
/// Returns `(start_byte, end_byte)` relative to the start of the data region.
pub fn validate_block_read(
    volume_shape: VolumeShape,
    mode: Mode,
    data_len: usize,
    offset: [usize; 3],
    block_shape: [usize; 3],
) -> Result<(usize, usize), Error> {
    let [nx, ny, nz] = [volume_shape.nx, volume_shape.ny, volume_shape.nz];
    let [ox, oy, oz] = offset;
    let [sx, sy, sz] = block_shape;

    if ox + sx > nx || oy + sy > ny || oz + sz > nz {
        return Err(Error::BoundsError);
    }

    if mode == Mode::Packed4Bit {
        return Err(Error::UnsupportedMode);
    }

    let linear = volume_shape.checked_linear_index(offset).ok_or(Error::BoundsError)?;
    let start_byte = linear
        .checked_mul(mode.byte_size())
        .ok_or(Error::BoundsError)?;
    let count = sx.checked_mul(sy).and_then(|v| v.checked_mul(sz))
        .ok_or(Error::BoundsError)?;
    let byte_len = mode.byte_size_for_count(count);
    let end_byte = start_byte.checked_add(byte_len).ok_or(Error::BoundsError)?;

    if end_byte > data_len {
        return Err(Error::BoundsError);
    }

    Ok((start_byte, end_byte))
}

/// Decode a byte block to the requested voxel type.
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

/// Native-endian decode: memcpy bytes directly to Vec<T>.
///
/// # Safety
/// This function uses `unsafe` to copy raw bytes into a typed Vec. The caller
/// must ensure that `bytes.len()` is an exact multiple of `T::BYTE_SIZE` and
/// that the byte pattern is valid for `T`. For MRC data this always holds
/// because the byte count is derived from `mode.byte_size() * count`.
pub fn decode_native_endian<T: EndianCodec + Copy>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    let n = bytes.len() / T::BYTE_SIZE;
    let mut result = Vec::with_capacity(n);
    unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            result.as_mut_ptr() as *mut u8,
            bytes.len(),
        );
        result.set_len(n);
    }
    Ok(result)
}

/// Build a `slices_f32` iterator from a generic byte reader.
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

/// Build a `slabs_f32` iterator from a generic byte reader.
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


impl private::Sealed for crate::Reader {}
impl VoxelSource for crate::Reader {
    fn vs_read_block_bytes<'a>(&'a self, offset: [usize; 3], shape: [usize; 3]) -> Result<Cow<'a, [u8]>, Error> {
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
    fn vs_read_block_bytes<'a>(&'a self, offset: [usize; 3], shape: [usize; 3]) -> Result<Cow<'a, [u8]>, Error> {
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
    fn vs_read_block_bytes<'a>(&'a self, offset: [usize; 3], shape: [usize; 3]) -> Result<Cow<'a, [u8]>, Error> {
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
    fn vs_read_block_bytes<'a>(&'a self, offset: [usize; 3], shape: [usize; 3]) -> Result<Cow<'a, [u8]>, Error> {
        self.read_block_bytes(offset, shape).map(Cow::Borrowed)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.decode_block(bytes)
    }
}

impl private::Sealed for crate::any_reader::MrcReader {}
impl VoxelSource for crate::any_reader::MrcReader {
    fn vs_read_block_bytes<'a>(&'a self, offset: [usize; 3], shape: [usize; 3]) -> Result<Cow<'a, [u8]>, Error> {
        self.read_block_bytes(offset, shape).map(Cow::Owned)
    }
    fn vs_decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        self.decode_block(bytes)
    }
}
