//! Internal helpers for block validation, endian decoding, and auto-conversion.
//!
//! This module contains the helper functions used by [`Reader`](crate::Reader)
//! internally, plus the [`ConvertReader`] wrapper for automatic mode conversion.

use crate::engine::codec::{EndianCodec, decode_slice, encode_slice};
use crate::engine::endian::FileEndian;
use crate::iter::{SlabStepper, SliceStepper, Stepper, TileStepper};
use crate::mode::Voxel;
use crate::{Error, Mode};
use crate::{VolumeShape, VoxelBlock};
use std::io::Read;

/// Internal helper: type-erased voxel block iterator.
pub(crate) type VoxelIter<'a, T> = Box<dyn Iterator<Item = Result<VoxelBlock<T>, Error>> + 'a>;

// ============================================================================
// ConvertReader — auto-conversion wrapper
// ============================================================================

/// A reader wrapper that auto-converts all voxel data to type `T`.
///
/// Created via [`Reader::convert`](crate::Reader::convert).
pub struct ConvertReader<'a, T> {
    pub(crate) reader: &'a crate::Reader,
    pub(crate) complex_strategy: crate::ComplexToRealStrategy,
    pub(crate) m0_interp: crate::M0Interpretation,
    pub(crate) _target: core::marker::PhantomData<T>,
}

impl<'a, T> ConvertReader<'a, T>
where
    T: Voxel + crate::engine::convert::ConvertFrom<f32>,
{
    pub fn slices(&self) -> VoxelIter<'_, T> {
        let shape = self.reader.shape();
        Box::new(convert_iter::<SliceStepper, T>(
            self.reader,
            shape,
            SliceStepper::default(),
            self.complex_strategy,
            self.m0_interp,
        ))
    }

    pub fn slabs(&self, k: usize) -> VoxelIter<'_, T> {
        let shape = self.reader.shape();
        Box::new(convert_iter::<SlabStepper, T>(
            self.reader,
            shape,
            SlabStepper::new(k),
            self.complex_strategy,
            self.m0_interp,
        ))
    }

    pub fn tiles(&self, tile_shape: [usize; 3]) -> Result<VoxelIter<'_, T>, Error> {
        let shape = self.reader.shape();
        Ok(Box::new(convert_iter::<TileStepper, T>(
            self.reader,
            shape,
            TileStepper::new(tile_shape)?,
            self.complex_strategy,
            self.m0_interp,
        )))
    }

    pub fn subregion(
        &self,
        offset: [usize; 3],
        block_shape: [usize; 3],
    ) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.reader.read_block_bytes_cow(offset, block_shape)?;
        let s = self.reader.shape();
        let data = crate::engine::convert::convert_block::<T>(
            &bytes,
            self.reader.mode(),
            self.reader.endian(),
            s.nx,
            s.ny,
            block_shape,
            self.complex_strategy,
            self.m0_interp,
        )?;
        Ok(VoxelBlock {
            offset,
            shape: block_shape,
            data,
        })
    }

    pub fn with_complex_strategy(mut self, strategy: crate::ComplexToRealStrategy) -> Self {
        self.complex_strategy = strategy;
        self
    }

    pub fn with_m0_interpretation(mut self, interp: crate::M0Interpretation) -> Self {
        self.m0_interp = interp;
        self
    }

    pub fn read_volume(&self) -> Result<VoxelBlock<T>, Error> {
        let s = self.reader.shape();
        let block_shape = [s.nx, s.ny, s.nz];
        self.subregion([0, 0, 0], block_shape)
    }

    #[cfg(feature = "ndarray")]
    pub fn to_ndarray(&self) -> Result<ndarray::Array3<T>, Error> {
        let block = self.read_volume()?;
        let s = self.reader.shape();
        ndarray::Array3::from_shape_vec([s.nz, s.ny, s.nx], block.data)
            .map_err(|_| Error::bounds_err())
    }
}

pub(crate) fn convert_iter<'a, S: Stepper + 'a, T>(
    reader: &'a crate::Reader,
    volume_shape: VolumeShape,
    stepper: S,
    complex_strategy: crate::ComplexToRealStrategy,
    m0_interp: crate::M0Interpretation,
) -> impl Iterator<Item = Result<VoxelBlock<T>, Error>> + 'a
where
    T: Voxel + crate::engine::convert::ConvertFrom<f32>,
{
    let mode = reader.mode();
    let endian = reader.endian();
    let nx = volume_shape.nx;
    let ny = volume_shape.ny;
    RawConvertIter::new(reader, volume_shape, stepper).map(move |result| {
        let (bytes, offset, shape) = result?;
        let data = crate::engine::convert::convert_block::<T>(
            &bytes,
            mode,
            endian,
            nx,
            ny,
            shape,
            complex_strategy,
            m0_interp,
        )?;
        Ok(VoxelBlock {
            offset,
            shape,
            data,
        })
    })
}

/// Raw-byte block iterator (used internally by convert_iter).
struct RawConvertIter<'a, S> {
    reader: &'a crate::Reader,
    volume_shape: VolumeShape,
    stepper: S,
}

impl<'a, S> RawConvertIter<'a, S> {
    fn new(reader: &'a crate::Reader, volume_shape: VolumeShape, stepper: S) -> Self {
        Self {
            reader,
            volume_shape,
            stepper,
        }
    }
}

impl<'a, S: Stepper> Iterator for RawConvertIter<'a, S> {
    type Item = Result<(Vec<u8>, [usize; 3], [usize; 3]), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, shape) = self.stepper.next(self.volume_shape)?;
        match self.reader.read_block_bytes_cow(offset, shape) {
            Ok(bytes) => Some(Ok((bytes.into_owned(), offset, shape))),
            Err(e) => Some(Err(e)),
        }
    }
}

// ============================================================================
// Block validation and I/O helpers
// ============================================================================

/// Cold path helper for bounds errors.
#[cold]
#[inline(never)]
fn cold_bounds_error() -> Error {
    Error::bounds_err()
}

/// Validate a block read/write request.
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

    let bounds_err = || Error::BoundsError {
        offset: Some(offset),
        shape: Some(block_shape),
        volume: Some([nx, ny, nz]),
    };

    if ox.checked_add(sx).is_none_or(|end| end > nx)
        || oy.checked_add(sy).is_none_or(|end| end > ny)
        || oz.checked_add(sz).is_none_or(|end| end > nz)
    {
        return Err(bounds_err());
    }

    let count = sx
        .checked_mul(sy)
        .and_then(|v| v.checked_mul(sz))
        .ok_or_else(bounds_err)?;
    let block_row_bytes = sx.div_ceil(2);
    let byte_len = if mode == Mode::Packed4Bit {
        block_row_bytes
            .checked_mul(sy)
            .and_then(|v| v.checked_mul(sz))
            .ok_or_else(bounds_err)?
    } else {
        mode.byte_size_for_count(count)
    };

    if count == 0 {
        return Ok(0);
    }

    if mode == Mode::Packed4Bit {
        if ox % 2 != 0 {
            return Err(bounds_err());
        }
        let vol_row_bytes = nx.div_ceil(2);
        let start_byte_in_row = ox / 2;
        let last_vol_row = (oz + sz - 1) * ny + (oy + sy - 1);
        let last_byte = last_vol_row
            .checked_mul(vol_row_bytes)
            .and_then(|b| b.checked_add(start_byte_in_row))
            .and_then(|b| b.checked_add(block_row_bytes))
            .ok_or_else(bounds_err)?;
        if last_byte > data_len {
            return Err(bounds_err());
        }
    } else {
        let last_row_start = volume_shape
            .checked_linear_index([ox, oy + sy - 1, oz + sz - 1])
            .ok_or_else(bounds_err)?;
        let last_byte = last_row_start
            .checked_add(sx)
            .map(|end| mode.byte_size_for_count(end))
            .ok_or_else(bounds_err)?;
        if last_byte > data_len {
            return Err(bounds_err());
        }
    }

    Ok(byte_len)
}

/// Gather a non-contiguous 3D block from raw data bytes.
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
        let vol_row_bytes = nx.div_ceil(2);
        let block_row_bytes = sx.div_ceil(2);
        let byte_len = block_row_bytes * sy * sz;
        let mut dst = vec![0u8; byte_len];

        if ox == 0 && sx == nx && oy == 0 && sy == ny {
            let slice_bytes = ny * vol_row_bytes;
            let start = oz * slice_bytes;
            let len = sz * slice_bytes;
            return data[start..start + len].to_vec();
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

    if ox == 0 && sx == nx && oy == 0 && sy == ny {
        let linear = oz * nx * ny;
        let start = linear * b;
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

/// Encode a typed voxel block into a mutable byte buffer.
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

    if ox == 0 && sx == nx && oy == 0 && sy == ny {
        let linear = oz * nx * ny;
        let start_byte = data_offset + linear * b;
        let byte_len = sx * sy * sz * b;
        let end_byte = start_byte + byte_len;
        if end_byte > buf.len() {
            return Err(cold_bounds_error());
        }
        encode_slice(&block.data, &mut buf[start_byte..end_byte], file_endian)?;
        return Ok(());
    }

    for z in 0..sz {
        for y in 0..sy {
            let file_linear = ox + (oy + y) * nx + (oz + z) * nx * ny;
            let file_start = data_offset + file_linear * b;
            let block_idx = y * sx + z * sx * sy;
            if block_idx + sx > block.data.len() {
                return Err(cold_bounds_error());
            }
            let row_values = &block.data[block_idx..block_idx + sx];
            let row_end = file_start + sx * b;
            if row_end > buf.len() {
                return Err(cold_bounds_error());
            }
            encode_slice(row_values, &mut buf[file_start..row_end], file_endian)?;
        }
    }
    Ok(())
}

/// Write packed bytes for Packed4Bit mode.
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
    let file_row_bytes = nx.div_ceil(2);
    let block_row_bytes = sx.div_ceil(2);
    assert!(ox == 0, "write_block_bytes requires ox == 0");

    if sx == nx && oy == 0 && sy == ny {
        let slice_bytes = ny * file_row_bytes;
        let start_byte = data_offset + oz * slice_bytes;
        let byte_len = sz * slice_bytes;
        if start_byte + byte_len > buf.len() {
            return Err(cold_bounds_error());
        }
        buf[start_byte..start_byte + byte_len].copy_from_slice(&packed[..byte_len]);
        return Ok(());
    }

    for z in 0..sz {
        for y in 0..sy {
            let vol_row = (oz + z) * ny + (oy + y);
            let file_start = data_offset + vol_row * file_row_bytes;
            let file_end = file_start + block_row_bytes;
            if file_end > buf.len() {
                return Err(cold_bounds_error());
            }
            let packed_start = (y + z * sy) * block_row_bytes;
            let packed_end = packed_start + block_row_bytes;
            if packed_end > packed.len() {
                return Err(cold_bounds_error());
            }
            buf[file_start..file_end].copy_from_slice(&packed[packed_start..packed_end]);
        }
    }
    Ok(())
}

/// Decode a raw byte block to the requested voxel type.
pub(crate) fn decode_block<T: Voxel>(
    bytes: &[u8],
    file_mode: Mode,
    endian: FileEndian,
) -> Result<Vec<T>, Error> {
    if T::MODE != file_mode {
        return Err(Error::ModeMismatch {
            file_mode,
            requested_mode: T::MODE,
            offset: None,
        });
    }
    if endian == FileEndian::native() {
        decode_native_endian(bytes)
    } else {
        decode_slice(bytes, endian)
    }
}

/// Fast native-endian decode: copy bytes directly into `Vec<T>`.
fn decode_native_endian<T: EndianCodec + Copy>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    let n = bytes.len() / T::BYTE_SIZE;
    debug_assert_eq!(bytes.len() % T::BYTE_SIZE, 0);
    let mut result = Vec::with_capacity(n);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), result.as_mut_ptr() as *mut u8, bytes.len());
        result.set_len(n);
    }
    Ok(result)
}

/// Parse and validate an MRC header from raw bytes.
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
    if let Some(w) = endian_warning {
        warnings.push(w.to_string());
    }
    let data_size = header.data_size().ok_or(crate::Error::InvalidHeader)?;
    let endian = header.detect_endian();
    Ok((header, warnings, endian, data_size))
}

/// Default maximum decompressed bytes for compressed MRC files (256 GiB).
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024 * 1024;

/// Components of a decompressed MRC file.
pub(crate) struct DecompressedMrc {
    pub header: crate::Header,
    pub ext_header: Vec<u8>,
    pub data: Vec<u8>,
    pub warnings: Vec<String>,
}

/// Open a compressed MRC file from a decoder.
pub(crate) fn open_compressed<D: std::io::Read>(
    mut decoder: D,
    permissive: bool,
    max_bytes: u64,
) -> Result<DecompressedMrc, crate::Error> {
    let limit = max_bytes.saturating_add(1);
    let mut buf = Vec::with_capacity(limit.min(1024 * 1024) as usize);
    decoder.by_ref().take(limit).read_to_end(&mut buf)?;

    if buf.len() > max_bytes as usize {
        return Err(crate::Error::Io(std::io::Error::other(format!(
            "Decompressed data exceeds safety limit of {max_bytes} bytes. \
             Use Reader::open_gzip_with_limit() with a larger max_bytes if you trust this file.",
        ))));
    }

    if buf.len() < 1024 {
        return Err(crate::Error::InvalidHeader);
    }

    let mut header_bytes = [0u8; 1024];
    header_bytes.copy_from_slice(&buf[..1024]);
    let (header, mut warnings, _endian, data_size) = parse_header(&header_bytes, permissive)?;
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
            buf.len(),
        ));
    }

    let ext_end = (1024 + ext_size).min(buf.len());
    let ext_header = buf[1024..ext_end].to_vec();
    let data = if ext_end < buf.len() {
        buf[ext_end..].to_vec()
    } else {
        Vec::new()
    };
    let _shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

    if let Some(mode) = Mode::from_i32(header.mode) {
        if mode == Mode::Int8 {
            if let Some(imod) = header.detect_imod() {
                if !imod.bytes_are_signed {
                    warnings.push(
                        "IMOD file with unsigned Mode 0 detected: use slices_mode0() \
                         or convert::<f32>() for correct values"
                            .into(),
                    );
                }
            }
        }
    }

    Ok(DecompressedMrc {
        header,
        ext_header,
        data,
        warnings,
    })
}
