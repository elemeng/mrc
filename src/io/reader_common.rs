//! Internal helpers for block validation, endian decoding, and auto-conversion.
//!
//! This module contains the helper functions used by [`Reader`](crate::Reader)
//! internally, plus the [`ConvertReader`] wrapper for automatic mode conversion.

use crate::iter::{SlabStepper, SliceStepper, Stepper, TileStepper};
use crate::mode::Voxel;
use crate::{Error, Mode};
use crate::{VolumeShape, VoxelBlock};
use std::borrow::Cow;

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
    /// Iterate over Z-slices, auto-converting each to `T`.
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

    /// Iterate over slabs of `k` Z-planes, auto-converting each to `T`.
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

    /// Iterate over 3D tiles of the given shape, auto-converting each to `T`.
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

    /// Iterate over sub-volumes in a volume stack, auto-converting each to `T`.
    ///
    /// Each sub-volume has shape `[nx, ny, mz]`. Returns
    /// [`Error::NotAVolumeStack`] if the file is not a volume stack.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let reader = mrc::Reader::open("stack.mrc")?;
    /// for vol in reader.convert::<f32>().volumes()? {
    ///     let block = vol?;
    ///     println!("Sub-volume at z={} has {} voxels", block.offset[2], block.data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn volumes(&self) -> Result<VoxelIter<'_, T>, Error> {
        let mz = self.reader.header().mz.max(0) as usize;
        if !self.reader.header().is_volume_stack() || mz == 0 {
            return Err(crate::Error::NotAVolumeStack {
                ispg: self.reader.header().ispg,
                mz: self.reader.header().mz,
            });
        }
        Ok(self.slabs(mz))
    }

    /// Read a single sub-region at `offset` with `block_shape`, auto-converting to `T`.
    pub fn subregion(
        &self,
        offset: [usize; 3],
        block_shape: [usize; 3],
    ) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.reader.read_block_bytes_cow(offset, block_shape)?;
        let data = crate::engine::convert::convert_block::<T>(
            &bytes,
            self.reader.mode(),
            self.reader.endian(),
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

    /// Configure how complex modes (3, 4) are reduced to real values.
    pub fn with_complex_strategy(mut self, strategy: crate::ComplexToRealStrategy) -> Self {
        self.complex_strategy = strategy;
        self
    }

    /// Configure how Mode 0 (Int8) is interpreted (signed or unsigned).
    pub fn with_m0_interpretation(mut self, interp: crate::M0Interpretation) -> Self {
        self.m0_interp = interp;
        self
    }

    /// Read the entire volume as a single block, auto-converting to `T`.
    pub fn read_volume(&self) -> Result<VoxelBlock<T>, Error> {
        let s = self.reader.shape();
        let block_shape = [s.nx, s.ny, s.nz];
        self.subregion([0, 0, 0], block_shape)
    }

    /// Read the entire volume as an `ndarray::Array3<T>`.
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
    RawConvertIter::new(reader, volume_shape, stepper).map(move |result| {
        let (bytes, offset, shape) = result?;
        let data = crate::engine::convert::convert_block::<T>(
            &bytes,
            mode,
            endian,
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
    type Item = Result<(Cow<'a, [u8]>, [usize; 3], [usize; 3]), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, shape) = self.stepper.next(self.volume_shape)?;
        match self.reader.read_block_bytes_cow(offset, shape) {
            Ok(bytes) => Some(Ok((bytes, offset, shape))),
            Err(e) => Some(Err(e)),
        }
    }
}

// ============================================================================
// Block validation and I/O helpers
// ============================================================================

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

        let base_row = oz * ny + oy;
        for z in 0..sz {
            let z_row_off = z * ny;
            for y in 0..sy {
                let vol_row = base_row + z_row_off + y;
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

    let z_stride = nx * ny;
    let row_stride = nx;
    let base_linear = ox + oy * nx + oz * nx * ny;
    for z in 0..sz {
        let z_off = z * z_stride;
        for y in 0..sy {
            let src_start = (base_linear + z_off + y * row_stride) * b;
            let dst_start = (y * sx + z * sx * sy) * b;
            dst[dst_start..dst_start + sx * b]
                .copy_from_slice(&data[src_start..src_start + sx * b]);
        }
    }
    dst
}

/// Warning message for IMOD files with unsigned Mode 0 data.
pub(crate) const IMOD_UNSIGNED_M0_WARNING: &str = "IMOD file with unsigned Mode 0 detected: use slices_mode0() \
     or convert::<f32>() for correct values";

/// Parse and validate an MRC header from raw bytes.
pub(crate) fn parse_header(
    header_bytes: &[u8],
    permissive: bool,
) -> Result<(crate::Header, Vec<String>, crate::FileEndian, usize), crate::Error> {
    let header_arr: &[u8; 1024] = header_bytes
        .try_into()
        .map_err(|_| crate::Error::InvalidHeader)?;
    let (header, endian_warning) = crate::Header::decode_from_bytes_with_info(header_arr);
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
///
/// The limit is checked against the **declared** file size in the header
/// before the voxel payload is allocated. For untrusted input, pass a smaller
/// limit to [`Reader::open_gzip_with_limit`](crate::Reader::open_gzip_with_limit)
/// or [`Reader::open_bzip2_with_limit`](crate::Reader::open_bzip2_with_limit)
/// that reflects the memory you are willing to commit.
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024 * 1024;

/// Components of a decompressed MRC file.
#[cfg(any(feature = "gzip", feature = "bzip2"))]
pub(crate) struct DecompressedMrc {
    pub header: crate::Header,
    pub ext_header: Vec<u8>,
    pub data: Vec<u8>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

/// Read until `buf` is full or the source reports EOF.
#[cfg(any(feature = "gzip", feature = "bzip2"))]
fn read_up_to<R: std::io::Read>(reader: &mut R, buf: &mut [u8]) -> Result<usize, crate::Error> {
    let mut n = 0;
    while n < buf.len() {
        match reader.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(n)
}

/// Open a compressed MRC file from a decoder.
///
/// The fixed header is parsed first so the declared file size can be checked
/// against `max_bytes` **before** the voxel payload is materialised. A stream
/// that continues past the declared end is probed one byte beyond it rather
/// than drained.
#[cfg(any(feature = "gzip", feature = "bzip2"))]
pub(crate) fn open_compressed<D: std::io::Read>(
    mut decoder: D,
    permissive: bool,
    max_bytes: u64,
) -> Result<DecompressedMrc, crate::Error> {
    let mut header_bytes = [0u8; 1024];
    let header_read = read_up_to(&mut decoder, &mut header_bytes)?;
    if header_read < 1024 {
        return Err(crate::Error::InvalidHeader);
    }

    let (header, mut warnings, _endian, data_size) = parse_header(&header_bytes, permissive)?;
    let ext_size = header.nsymbt.max(0) as usize;
    let expected = 1024usize
        .checked_add(ext_size)
        .and_then(|v| v.checked_add(data_size))
        .ok_or(crate::Error::InvalidHeader)?;

    if expected as u64 > max_bytes {
        return Err(crate::Error::DataTooLarge { limit: max_bytes });
    }

    let mut ext_header = vec![0u8; ext_size];
    let ext_read = read_up_to(&mut decoder, &mut ext_header)?;
    if ext_read < ext_size {
        if !permissive {
            return Err(crate::Error::FileSizeMismatch {
                expected,
                actual: 1024 + ext_read,
            });
        }
        ext_header.truncate(ext_read);
        warnings.push(format!(
            "Extended header truncated: expected {ext_size} bytes, got {ext_read}"
        ));
    }

    let mut data = vec![0u8; data_size];
    let data_read = read_up_to(&mut decoder, &mut data)?;
    data.truncate(data_read);

    let actual = 1024 + ext_header.len() + data.len();
    if data_read < data_size {
        if !permissive {
            return Err(crate::Error::FileSizeMismatch { expected, actual });
        }
        warnings.push(format!(
            "File size mismatch: expected {expected} bytes, got {actual}"
        ));
    } else {
        // Probe for trailing bytes without draining the rest of the stream.
        let mut probe = [0u8; 1];
        let extra = read_up_to(&mut decoder, &mut probe)?;
        if extra > 0 {
            if !permissive {
                return Err(crate::Error::FileSizeMismatch {
                    expected,
                    actual: expected.saturating_add(1),
                });
            }
            warnings.push(format!("File has trailing data after {expected} bytes"));
        }
    }

    Ok(DecompressedMrc {
        header,
        ext_header,
        data,
        warnings,
        truncated: data_read < data_size,
    })
}
