//! MRC-specific type conversions.
//!
//! This module provides the generic conversion trait [`ConvertFrom`] that
//! powers the unified reader conversion system.
//!
//! Specific conversions:
//! - `i8`/`i16`/`u16`/`u8` → `f32` (for `convert::<f32>()` auto-conversion)
//! - `f16` ↔ `f32` (for `convert::<f32>()` auto-conversion and `write_block_as`)
//! - `u8` → `u16`, `u16` → `u8` (Mode 6 utilities)
//! - Mode 0 reinterpretation (signed vs unsigned `i8`)
//! - 4-bit packed data unpacking/packing

use crate::Voxel;
use crate::mode::M0Interpretation;

/// Reinterpret a `Vec<S>` as `Vec<T>` without copying.
///
/// # Safety
/// The caller must ensure that `S` and `T` have the same size and alignment,
/// and that the byte pattern of `S` is valid for `T`.  This is satisfied when
/// the caller has verified `TypeId::of::<S>() == TypeId::of::<T>()` (which
/// guarantees `S` and `T` are the same type at the monomorphized call site).
unsafe fn reinterpret_vec<S, T>(v: Vec<S>) -> Vec<T> {
    debug_assert_eq!(core::mem::size_of::<S>(), core::mem::size_of::<T>());
    debug_assert_eq!(core::mem::align_of::<S>(), core::mem::align_of::<T>());
    let ptr = v.as_ptr() as *mut T;
    let len = v.len();
    let cap = v.capacity();
    core::mem::forget(v);
    // SAFETY: This function is itself `unsafe`; the caller must uphold the
    // invariants documented above.  The debug_assert_eq! calls above verify
    // size and alignment parity at runtime in debug builds.
    unsafe { Vec::from_raw_parts(ptr, len, cap) }
}

#[cfg(feature = "simd")]
use super::simd;

use super::codec::decode_slice;
use super::endian::FileEndian;
use crate::Error;
use crate::mode::{ComplexToRealStrategy, Float32Complex, Int16Complex, Mode};

// ============================================================================
// Generic conversion traits
// ============================================================================

/// Convert from a source voxel type to `Self` (reader side).
///
/// Used by [`convert_block`] to dispatch per-mode conversions at runtime.
/// The source type is determined by the file's on-disk mode; `Self` is the
/// target type requested by the caller.
///
/// Only the following target types are wired up:
/// - **`f32`** — universal target, zero-copy identity when source is Float32
/// - **`f16`** — via f32 hub (SIMD F16C/NEON), requires `f16` feature
/// - **`i16`** — shortcut `i8↔i16`, `u16↔i16`; f32 hub for all other sources
/// - **`u16`** — shortcut `i8↔u16`, `i16↔u16`; f32 hub for all other sources
/// - **`i8`** — shortcut `i16↔i8`, `u16↔i8`; f32 hub for all other sources
///
/// # Identity
/// The blanket `impl<T: Voxel> ConvertFrom<T> for T` handles the case
/// where source and target are the same type (no conversion needed).
pub trait ConvertFrom<Src: Voxel>: Voxel {
    /// Convert a slice of source voxels to `Self`.
    fn convert_from(src: &[Src]) -> Vec<Self>;
}

/// Identity conversion: same source and target, just copy.
impl<T: Voxel> ConvertFrom<T> for T {
    fn convert_from(src: &[T]) -> Vec<T> {
        src.to_vec()
    }
}

// ============================================================================
// ConvertFrom implementations (reader side — any source → target)
// ============================================================================

impl ConvertFrom<i16> for f32 {
    fn convert_from(src: &[i16]) -> Vec<f32> {
        convert_i16_slice_to_f32(src)
    }
}

impl ConvertFrom<i8> for f32 {
    fn convert_from(src: &[i8]) -> Vec<f32> {
        convert_i8_slice_to_f32(src)
    }
}

impl ConvertFrom<u16> for f32 {
    fn convert_from(src: &[u16]) -> Vec<f32> {
        convert_u16_slice_to_f32(src)
    }
}

#[cfg(feature = "f16")]
impl ConvertFrom<crate::f16> for f32 {
    fn convert_from(src: &[crate::f16]) -> Vec<f32> {
        convert_f16_slice_to_f32(src)
    }
}

/// Reverse conversion: f32 → f16 for the reader-side convert API.
///
/// Enables `reader.convert::<f16>()` for any source mode (reads, converts
/// through f32 intermediate, then narrows to f16).
#[cfg(feature = "f16")]
impl ConvertFrom<f32> for crate::f16 {
    fn convert_from(src: &[f32]) -> Vec<crate::f16> {
        convert_f32_slice_to_f16(src)
    }
}

/// f32 → i16 for the reader-side convert API.
///
/// Enables `reader.convert::<i16>()` for any source mode via the f32 hub.
/// Uses SIMD when available (see [`convert_f32_slice_to_i16`]).
impl ConvertFrom<f32> for i16 {
    fn convert_from(src: &[f32]) -> Vec<i16> {
        convert_f32_slice_to_i16(src)
    }
}

/// f32 → u16 for the reader-side convert API.
///
/// Enables `reader.convert::<u16>()` for any source mode via the f32 hub.
/// Uses SIMD when available (see [`convert_f32_slice_to_u16`]).
impl ConvertFrom<f32> for u16 {
    fn convert_from(src: &[f32]) -> Vec<u16> {
        convert_f32_slice_to_u16(src)
    }
}

/// f32 → i8 for the reader-side convert API.
///
/// Enables `reader.convert::<i8>()` for any source mode via the f32 hub.
/// Uses SIMD when available (see [`convert_f32_slice_to_i8`]).
impl ConvertFrom<f32> for i8 {
    fn convert_from(src: &[f32]) -> Vec<i8> {
        convert_f32_slice_to_i8(src)
    }
}

// === Packed4Bit (Mode 101) — row-by-row unpack/pack ===

/// Unpack 4-bit packed bytes to `u8`, row-by-row.
///
/// Each row has `nx.div_ceil(2)` bytes in the source.  When `nx` is odd, the
/// last byte's high nibble is padding and is ignored.
///
/// `ny` is the total number of rows (i.e. `ny * nz` for a 3D volume).
///
/// # Nibble ordering (SerialEM convention)
/// - Low 4 bits  (bit 0–3) = first pixel  (smaller X coordinate)
/// - High 4 bits (bit 4–7) = second pixel (larger X coordinate)
pub(crate) fn unpack_u4_bytes_to_u8(src: &[u8], nx: usize, ny: usize) -> Vec<u8> {
    let row_bytes = nx.div_ceil(2);
    let mut dst = Vec::with_capacity(nx * ny);
    for y in 0..ny {
        let row_start = y * row_bytes;
        for x in 0..nx {
            let byte = src[row_start + x / 2];
            let nibble = if x % 2 == 0 {
                byte & 0x0F
            } else {
                (byte >> 4) & 0x0F
            };
            dst.push(nibble);
        }
    }
    dst
}

/// Pack `u8` values (0–15) into 4-bit packed bytes, row-by-row.
///
/// Each row produces `nx.div_ceil(2)` bytes.  When `nx` is odd, the
/// padding high nibble is zero-filled.
///
/// `ny` is the total number of rows (i.e. `ny * nz` for a 3D volume).
///
/// Values exceeding 15 are silently masked to 4 bits (`val & 0x0F`).
/// The caller should validate values beforehand (e.g. in `write_u4_block`).
pub(crate) fn pack_u8_to_u4_bytes(src: &[u8], nx: usize, ny: usize) -> Vec<u8> {
    let row_bytes = nx.div_ceil(2);
    let mut dst = vec![0u8; row_bytes * ny];
    for y in 0..ny {
        let row_start = y * row_bytes;
        for x in 0..nx {
            let val = src[y * nx + x] & 0x0F;
            let byte_idx = row_start + x / 2;
            if x % 2 == 0 {
                dst[byte_idx] = val;
            } else {
                dst[byte_idx] |= val << 4;
            }
        }
    }
    dst
}

/// Reinterpret Mode 0 (8-bit) data as signed or unsigned and convert to `f32`.
pub fn reinterpret_m0(data: &[u8], interp: M0Interpretation) -> Vec<f32> {
    match interp {
        M0Interpretation::Signed => {
            // SAFETY: `u8` and `i8` have the same size and alignment; the byte
            // pattern is valid for `i8` because every bit pattern is valid for `i8`.
            let src: &[i8] =
                unsafe { core::slice::from_raw_parts(data.as_ptr() as *const i8, data.len()) };
            convert_i8_slice_to_f32(src)
        }
        M0Interpretation::Unsigned => convert_u8_slice_to_f32(data),
    }
}

// === Batch slice conversions (used by convert::<f32>().slices()) ===

/// Batch conversion from i8 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub(crate) fn convert_i8_slice_to_f32(src: &[i8]) -> Vec<f32> {
    simd::convert_i8_to_f32_simd(src)
}

/// Batch conversion from i8 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_i8_slice_to_f32(src: &[i8]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from i16 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub(crate) fn convert_i16_slice_to_f32(src: &[i16]) -> Vec<f32> {
    simd::convert_i16_to_f32_simd(src)
}

/// Batch conversion from i16 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_i16_slice_to_f32(src: &[i16]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from u16 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub(crate) fn convert_u16_slice_to_f32(src: &[u16]) -> Vec<f32> {
    simd::convert_u16_to_f32_simd(src)
}

/// Batch conversion from u16 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_u16_slice_to_f32(src: &[u16]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from u8 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub(crate) fn convert_u8_slice_to_f32(src: &[u8]) -> Vec<f32> {
    simd::convert_u8_to_f32_simd(src)
}

/// Batch conversion from u8 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_u8_slice_to_f32(src: &[u8]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from f16 to f32 using SIMD when available.
#[cfg(all(feature = "simd", feature = "f16"))]
pub(crate) fn convert_f16_slice_to_f32(src: &[crate::f16]) -> Vec<f32> {
    simd::convert_f16_to_f32_simd(src)
}

/// Batch conversion from f16 to f32 (scalar fallback).
#[cfg(all(feature = "f16", not(feature = "simd")))]
pub(crate) fn convert_f16_slice_to_f32(src: &[crate::f16]) -> Vec<f32> {
    src.iter().map(|&v| f32::from(v)).collect()
}

/// Batch conversion from f32 to f16 using SIMD when available.
#[cfg(all(feature = "simd", feature = "f16"))]
pub(crate) fn convert_f32_slice_to_f16(src: &[f32]) -> Vec<crate::f16> {
    simd::convert_f32_to_f16_simd(src)
}

/// Batch conversion from f32 to f16 (scalar fallback).
#[cfg(all(feature = "f16", not(feature = "simd")))]
pub(crate) fn convert_f32_slice_to_f16(src: &[f32]) -> Vec<crate::f16> {
    src.iter().map(|&v| crate::f16::from_f32(v)).collect()
}

// ============================================================================
// Write-side conversions (f32 → integer types)
// ============================================================================

/// Convert `f32` values to `i16`, clamping to the representable range.
#[cfg(feature = "simd")]
pub(crate) fn convert_f32_slice_to_i16(src: &[f32]) -> Vec<i16> {
    simd::convert_f32_to_i16_simd(src)
}

/// Convert `f32` values to `i16`, clamping to the representable range.
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_f32_slice_to_i16(src: &[f32]) -> Vec<i16> {
    src.iter()
        .map(|&v| {
            if v >= i16::MAX as f32 {
                i16::MAX
            } else if v <= i16::MIN as f32 {
                i16::MIN
            } else {
                v as i16
            }
        })
        .collect()
}

/// Convert `f32` values to `u16`, clamping to the representable range.
/// Negative values are clamped to 0.
#[cfg(feature = "simd")]
pub(crate) fn convert_f32_slice_to_u16(src: &[f32]) -> Vec<u16> {
    simd::convert_f32_to_u16_simd(src)
}

/// Convert `f32` values to `u16`, clamping to the representable range.
/// Negative values are clamped to 0.
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_f32_slice_to_u16(src: &[f32]) -> Vec<u16> {
    src.iter()
        .map(|&v| {
            if v >= u16::MAX as f32 {
                u16::MAX
            } else if v <= 0.0 {
                0
            } else {
                v as u16
            }
        })
        .collect()
}

/// Convert `f32` values to `i8`, clamping to the representable range.
#[cfg(feature = "simd")]
pub(crate) fn convert_f32_slice_to_i8(src: &[f32]) -> Vec<i8> {
    simd::convert_f32_to_i8_simd(src)
}

/// Convert `f32` values to `i8`, clamping to the representable range.
#[cfg(not(feature = "simd"))]
pub(crate) fn convert_f32_slice_to_i8(src: &[f32]) -> Vec<i8> {
    src.iter()
        .map(|&v| {
            if v >= i8::MAX as f32 {
                i8::MAX
            } else if v <= i8::MIN as f32 {
                i8::MIN
            } else {
                v as i8
            }
        })
        .collect()
}

// ============================================================================
// Integer↔integer direct conversions (avoid f32 intermediate)
// ============================================================================

/// Widen `i8` to `i16` (always exact, no clamping needed).
pub(crate) fn convert_i8_slice_to_i16(src: &[i8]) -> Vec<i16> {
    src.iter().map(|&v| v as i16).collect()
}

/// Narrow `i16` to `i8`, clamping to the representable range.
pub(crate) fn convert_i16_slice_to_i8(src: &[i16]) -> Vec<i8> {
    src.iter()
        .map(|&v| {
            if v > i8::MAX as i16 {
                i8::MAX
            } else if v < i8::MIN as i16 {
                i8::MIN
            } else {
                v as i8
            }
        })
        .collect()
}

/// Convert `i8` to `u16` (negative values become 0, positive widen exactly).
pub(crate) fn convert_i8_slice_to_u16(src: &[i8]) -> Vec<u16> {
    src.iter().map(|&v| v.max(0) as u16).collect()
}

/// Convert `u16` to `i8`, clamping to the representable range.
pub(crate) fn convert_u16_slice_to_i8(src: &[u16]) -> Vec<i8> {
    src.iter()
        .map(|&v| if v > i8::MAX as u16 { i8::MAX } else { v as i8 })
        .collect()
}

/// Convert `i16` to `u16` (negative values become 0).
pub(crate) fn convert_i16_slice_to_u16(src: &[i16]) -> Vec<u16> {
    src.iter().map(|&v| v.max(0) as u16).collect()
}

/// Convert `u16` to `i16`, clamping to the representable range.
pub(crate) fn convert_u16_slice_to_i16(src: &[u16]) -> Vec<i16> {
    src.iter()
        .map(|&v| {
            if v > i16::MAX as u16 {
                i16::MAX
            } else {
                v as i16
            }
        })
        .collect()
}

// ============================================================================
// Generic conversion dispatcher — single match over all source modes
// ============================================================================

/// Decode a raw byte block to its native MRC type, dispatching at runtime.
///
/// Returns [`OwnedData`] with the correct typed `Vec` for the file's mode.
/// This is the runtime-dispatched counterpart of [`decode_block`] which
/// requires a compile-time type parameter.
///
/// For native-endian data this is a simple memcpy.  For non-native endian
/// it decodes element-by-element with byte swapping.
pub(crate) fn decode_block_to_any(
    bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
    _block_shape: [usize; 3],
) -> Result<crate::mode::OwnedData, Error> {
    Ok(match mode {
        Mode::Int8 => {
            let src = decode_slice::<i8>(bytes, endian)?;
            crate::mode::OwnedData::Int8(src)
        }
        Mode::Int16 => {
            let src = decode_slice::<i16>(bytes, endian)?;
            crate::mode::OwnedData::Int16(src)
        }
        Mode::Float32 => {
            let src = decode_slice::<f32>(bytes, endian)?;
            crate::mode::OwnedData::Float32(src)
        }
        Mode::Int16Complex => {
            let src = decode_slice::<Int16Complex>(bytes, endian)?;
            crate::mode::OwnedData::Int16Complex(src)
        }
        Mode::Float32Complex => {
            let src = decode_slice::<Float32Complex>(bytes, endian)?;
            crate::mode::OwnedData::Float32Complex(src)
        }
        Mode::Uint16 => {
            let src = decode_slice::<u16>(bytes, endian)?;
            crate::mode::OwnedData::Uint16(src)
        }
        #[cfg(feature = "f16")]
        Mode::Float16 => {
            let src = decode_slice::<crate::f16>(bytes, endian)?;
            crate::mode::OwnedData::Float16(src)
        }
        #[cfg(not(feature = "f16"))]
        Mode::Float16 => return Err(Error::UnsupportedMode),
        Mode::Packed4Bit => {
            // Packed4Bit data is stored as raw bytes; no endian conversion needed.
            crate::mode::OwnedData::Packed4Bit(bytes.to_vec())
        }
    })
}

/// Convert a raw byte slice from any MRC mode to target type `T`.
///
/// This is the single dispatch point for all reader-side conversions.
/// The source mode is determined at runtime (from the file's header);
/// the target type `T` is a compile-time generic.
///
/// # Parameters
/// - `bytes` — raw voxel data bytes for the block
/// - `mode` — the file's on-disk mode
/// - `endian` — detected file endianness
/// - `block_shape` — dimensions `[sx, sy, sz]` of the block.  For Packed4Bit
///   this is used to compute the nibble-unpack row stride (`sx`) and total
///   row count (`sy × sz`).  For other modes it is unused.
///
/// # Dispatch
/// 1. **Direct integer shortcut** — when source and target are both narrow
///    integers (`i8↔i16`, `i8↔u16`, `i16↔u16`), the conversion skips the
///    f32 intermediate entirely, saving one allocation.
/// 2. **f32 hub fallback** — every other source mode (Float16, Float32,
///    complex, Packed4Bit) is decoded to `Vec<f32>` first, then converted
///    to `T` via [`ConvertFrom<f32>`]. Complex modes use the given
///    `complex_strategy` (default: [`Magnitude`](ComplexToRealStrategy::Magnitude)).
#[allow(clippy::too_many_arguments)]
pub(crate) fn convert_block<T>(
    bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
    nx: usize,
    ny: usize,
    block_shape: [usize; 3],
    complex_strategy: ComplexToRealStrategy,
    m0_interp: M0Interpretation,
) -> Result<Vec<T>, Error>
where
    T: Voxel + ConvertFrom<f32>,
{
    // Direct integer↔integer shortcuts — avoids the f32 intermediate
    // (which would add 4N bytes of intermediate storage for narrow types).
    // i16 → i8 shortcut also handles the m0_interp distinction for i8 target
    // (i8 is always treated as signed in the integer domain; unsigned Mode 0
    //  is a byte-interpretation issue that only matters in the f32 hub).
    {
        // i16 → i8
        if mode == Mode::Int16 && core::any::TypeId::of::<T>() == core::any::TypeId::of::<i8>() {
            let src = decode_slice::<i16>(bytes, endian)?;
            let r = convert_i16_slice_to_i8(&src);
            // SAFETY: TypeId checked above guarantees T == i8, so sizes match.
            return Ok(unsafe { reinterpret_vec::<i8, T>(r) });
        }
        // i8 → i16
        if mode == Mode::Int8 && core::any::TypeId::of::<T>() == core::any::TypeId::of::<i16>() {
            let src = decode_slice::<i8>(bytes, endian)?;
            let r = convert_i8_slice_to_i16(&src);
            // SAFETY: TypeId checked above guarantees T == i16, so sizes match.
            return Ok(unsafe { reinterpret_vec::<i16, T>(r) });
        }
        // u16 → i8
        if mode == Mode::Uint16 && core::any::TypeId::of::<T>() == core::any::TypeId::of::<i8>() {
            let src = decode_slice::<u16>(bytes, endian)?;
            let r = convert_u16_slice_to_i8(&src);
            // SAFETY: TypeId checked above guarantees T == i8, so sizes match.
            return Ok(unsafe { reinterpret_vec::<i8, T>(r) });
        }
        // i8 → u16
        if mode == Mode::Int8 && core::any::TypeId::of::<T>() == core::any::TypeId::of::<u16>() {
            let src = decode_slice::<i8>(bytes, endian)?;
            let r = convert_i8_slice_to_u16(&src);
            // SAFETY: TypeId checked above guarantees T == u16, so sizes match.
            return Ok(unsafe { reinterpret_vec::<u16, T>(r) });
        }
        // u16 → i16
        if mode == Mode::Uint16 && core::any::TypeId::of::<T>() == core::any::TypeId::of::<i16>() {
            let src = decode_slice::<u16>(bytes, endian)?;
            let r = convert_u16_slice_to_i16(&src);
            // SAFETY: TypeId checked above guarantees T == i16, so sizes match.
            return Ok(unsafe { reinterpret_vec::<i16, T>(r) });
        }
        // i16 → u16
        if mode == Mode::Int16 && core::any::TypeId::of::<T>() == core::any::TypeId::of::<u16>() {
            let src = decode_slice::<i16>(bytes, endian)?;
            let r = convert_i16_slice_to_u16(&src);
            // SAFETY: TypeId checked above guarantees T == u16, so sizes match.
            return Ok(unsafe { reinterpret_vec::<u16, T>(r) });
        }
    }

    // Fall back to f32 hub
    //
    // Packed4Bit is handled here (not in convert_block_inner) because the
    // nibble-unpack step needs the block's actual dimensions (sx, sy × sz),
    // not the volume's (nx, ny).  Passing volume dims would miscompute the
    // row stride and row count for sub-block or multi-slice reads.
    let f32_data = match mode {
        Mode::Packed4Bit => {
            let sx = block_shape[0];
            let total_rows = block_shape[1] * block_shape[2];
            let unpacked = unpack_u4_bytes_to_u8(bytes, sx, total_rows);
            convert_u8_slice_to_f32(&unpacked)
        }
        Mode::Float16 => convert_block_float16(bytes, endian)?,
        other => convert_block_inner(bytes, other, endian, nx, ny, complex_strategy, m0_interp)?,
    };
    // Avoid the identity-clone when T == f32 by reusing the allocation.
    // SAFETY: The TypeId check guarantees T and f32 are the same type at the
    // monomorphized call site; the compiler optimizes the branch away.
    if core::any::TypeId::of::<T>() == core::any::TypeId::of::<f32>() {
        let ptr = f32_data.as_ptr() as *mut T;
        let len = f32_data.len();
        let cap = f32_data.capacity();
        core::mem::forget(f32_data);
        Ok(unsafe { Vec::from_raw_parts(ptr, len, cap) })
    } else {
        Ok(T::convert_from(&f32_data))
    }
}

/// Shared handler for the 6 modes that do not depend on the `f16` feature.
/// Always produces `Vec<f32>` as the intermediate representation.
///
/// Packed4Bit is handled in [`convert_block`] instead, because its nibble
/// unpack needs the block's actual dimensions, not the volume's.
fn convert_block_inner(
    bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
    _nx: usize,
    _ny: usize,
    complex_strategy: ComplexToRealStrategy,
    m0_interp: M0Interpretation,
) -> Result<Vec<f32>, Error> {
    match mode {
        Mode::Int8 => match m0_interp {
            M0Interpretation::Signed => {
                let src = decode_slice::<i8>(bytes, endian)?;
                Ok(convert_i8_slice_to_f32(&src))
            }
            M0Interpretation::Unsigned => Ok(reinterpret_m0(bytes, M0Interpretation::Unsigned)),
        },
        Mode::Int16 => {
            let src = decode_slice::<i16>(bytes, endian)?;
            Ok(convert_i16_slice_to_f32(&src))
        }
        Mode::Uint16 => {
            let src = decode_slice::<u16>(bytes, endian)?;
            Ok(convert_u16_slice_to_f32(&src))
        }
        Mode::Float32 => decode_slice::<f32>(bytes, endian),
        Mode::Float32Complex => {
            let src = decode_slice::<Float32Complex>(bytes, endian)?;
            let mag: Vec<f32> = src.iter().map(|c| c.to_real(complex_strategy)).collect();
            Ok(mag)
        }
        Mode::Int16Complex => {
            let src = decode_slice::<Int16Complex>(bytes, endian)?;
            let mag: Vec<f32> = src.iter().map(|c| c.to_real(complex_strategy)).collect();
            Ok(mag)
        }
        Mode::Packed4Bit => {
            // SAFETY: `convert_block` handles Packed4Bit before falling through to
            // `convert_block_inner`, so this arm is truly unreachable. If you refactor
            // the dispatch in `convert_block`, ensure Packed4Bit is still intercepted
            // first or update this arm accordingly.
            unreachable!("Packed4Bit is dispatched via convert_block before convert_block_inner")
        }
        Mode::Float16 => {
            // SAFETY: `convert_block` handles Float16 via `convert_block_float16` before
            // falling through to `convert_block_inner`, so this arm is truly unreachable.
            unreachable!(
                "Float16 is dispatched via convert_block_float16 before convert_block_inner"
            )
        }
    }
}

/// Handle Float16 mode conversion, which depends on the `f16` feature.
/// Always returns `Vec<f32>` — the final conversion to `T` happens in
/// [`convert_block`].
#[cfg(feature = "f16")]
fn convert_block_float16(bytes: &[u8], endian: FileEndian) -> Result<Vec<f32>, Error> {
    let src = decode_slice::<crate::f16>(bytes, endian)?;
    Ok(convert_f16_slice_to_f32(&src))
}

/// Float16 conversion unavailable — requires the `f16` feature.
#[cfg(not(feature = "f16"))]
fn convert_block_float16(_bytes: &[u8], _endian: FileEndian) -> Result<Vec<f32>, Error> {
    Err(Error::UnsupportedMode)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComplexToRealStrategy;

    // Test batch conversions
    #[test]
    fn test_convert_i8_slice_to_f32() {
        let input: Vec<i8> = vec![-128, -64, 0, 64, 127];
        let output = convert_i8_slice_to_f32(&input);

        assert_eq!(output.len(), input.len());
        for (src, dst) in input.iter().zip(output.iter()) {
            assert_eq!(*dst, *src as f32);
        }
    }

    #[test]
    fn test_convert_i16_slice_to_f32() {
        let input: Vec<i16> = vec![-32768, -1000, 0, 1000, 32767];
        let output = convert_i16_slice_to_f32(&input);

        assert_eq!(output.len(), input.len());
        for (src, dst) in input.iter().zip(output.iter()) {
            assert_eq!(*dst, *src as f32);
        }
    }

    #[test]
    fn test_convert_u16_slice_to_f32() {
        let input: Vec<u16> = vec![0, 1000, 32767, 65535];
        let output = convert_u16_slice_to_f32(&input);

        assert_eq!(output.len(), input.len());
        for (src, dst) in input.iter().zip(output.iter()) {
            assert_eq!(*dst, *src as f32);
        }
    }

    // Test edge cases
    #[test]
    fn test_convert_empty_slice() {
        let input: Vec<i8> = vec![];
        let output = convert_i8_slice_to_f32(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_convert_single_element() {
        let input: Vec<i16> = vec![42];
        let output = convert_i16_slice_to_f32(&input);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0], 42.0f32);
    }

    #[test]
    fn test_convert_large_slice() {
        let input: Vec<i16> = (0..10000).map(|i| (i % 65536) as i16).collect();
        let output = convert_i16_slice_to_f32(&input);

        assert_eq!(output.len(), input.len());
        for (src, dst) in input.iter().zip(output.iter()) {
            assert_eq!(*dst, *src as f32);
        }
    }

    // Test that SIMD and scalar paths produce identical results
    #[test]
    #[cfg(feature = "simd")]
    fn test_simd_scalar_equivalence_i8() {
        let input: Vec<i8> = (-128..=127).collect();
        let simd_result = crate::engine::convert::convert_i8_slice_to_f32(&input);
        let scalar_result: Vec<f32> = input.iter().map(|&x| x as f32).collect();
        assert_eq!(simd_result, scalar_result);
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_simd_scalar_equivalence_i16() {
        let input: Vec<i16> = (-32768..=-31768).collect(); // Full i16 range would be slow
        let simd_result = crate::engine::convert::convert_i16_slice_to_f32(&input);
        let scalar_result: Vec<f32> = input.iter().map(|&x| x as f32).collect();
        assert_eq!(simd_result, scalar_result);
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_simd_scalar_equivalence_u16() {
        let input: Vec<u16> = (0..10000).collect();
        let simd_result = crate::engine::convert::convert_u16_slice_to_f32(&input);
        let scalar_result: Vec<f32> = input.iter().map(|&x| x as f32).collect();
        assert_eq!(simd_result, scalar_result);
    }

    // Test M101 unpacking
    #[test]
    fn test_unpack_u4_bytes_to_u8_even() {
        let bytes = vec![0x21, 0x43];
        let result = unpack_u4_bytes_to_u8(&bytes, 4, 1);
        // row: [0x21, 0x43]
        // pixel 0: low of 0x21 = 1
        // pixel 1: high of 0x21 = 2
        // pixel 2: low of 0x43 = 3
        // pixel 3: high of 0x43 = 4
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_unpack_u4_bytes_to_u8_odd() {
        // nx=3 → row_bytes = 2; last byte's high nibble is padding
        let bytes = vec![0x21, 0x30]; // low of 0x30 = 0 is the 3rd pixel, high 0x30=3 is padding
        let result = unpack_u4_bytes_to_u8(&bytes, 3, 1);
        // pixel 0: low of 0x21 = 1
        // pixel 1: high of 0x21 = 2
        // pixel 2: low of 0x30 = 0
        assert_eq!(result, vec![1, 2, 0]);
    }

    #[test]
    fn test_pack_u8_to_u4_bytes_even() {
        let values = vec![1, 2, 3, 4];
        let packed = pack_u8_to_u4_bytes(&values, 4, 1);
        assert_eq!(packed, vec![0x21, 0x43]);
    }

    #[test]
    fn test_pack_u8_to_u4_bytes_odd() {
        let values = vec![1, 2, 3];
        let packed = pack_u8_to_u4_bytes(&values, 3, 1);
        // row_bytes = 2; byte0 = 1 | (2 << 4) = 0x21; byte1 = 3 | (0 << 4) = 0x03
        assert_eq!(packed, vec![0x21, 0x03]);
    }

    #[test]
    fn test_pack_unpack_roundtrip() {
        let values: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let packed = pack_u8_to_u4_bytes(&values, 8, 2);
        let unpacked = unpack_u4_bytes_to_u8(&packed, 8, 2);
        assert_eq!(unpacked, values);
    }

    #[test]
    fn test_pack_unpack_roundtrip_odd() {
        let values: Vec<u8> = vec![1, 2, 3, 4, 5]; // nx=5, ny=1 → 5 pixels, 3 bytes
        let packed = pack_u8_to_u4_bytes(&values, 5, 1);
        let unpacked = unpack_u4_bytes_to_u8(&packed, 5, 1);
        assert_eq!(unpacked, values);
    }

    // Test M0 reinterpretation
    #[test]
    fn test_reinterpret_m0_signed() {
        let data = vec![0x00, 0x80, 0xFF]; // 0, -128, -1 in signed i8
        let result = reinterpret_m0(&data, M0Interpretation::Signed);
        assert_eq!(result, vec![0.0, -128.0, -1.0]);
    }

    #[test]
    fn test_reinterpret_m0_unsigned() {
        let data = vec![0x00, 0x80, 0xFF]; // 0, 128, 255 in unsigned u8
        let result = reinterpret_m0(&data, M0Interpretation::Unsigned);
        assert_eq!(result, vec![0.0, 128.0, 255.0]);
    }

    // Test ComplexToRealStrategy
    #[test]
    fn test_complex_to_real_strategies() {
        let c = crate::mode::Float32Complex {
            real: 3.0,
            imag: 4.0,
        };
        assert_eq!(c.to_real(ComplexToRealStrategy::RealPart), 3.0);
        assert_eq!(c.to_real(ComplexToRealStrategy::ImaginaryPart), 4.0);
        assert_eq!(c.to_real(ComplexToRealStrategy::Magnitude), 5.0);
        let phase = c.to_real(ComplexToRealStrategy::Phase);
        assert!((phase - 0.927_295_2).abs() < 1e-6);
    }
}

// ============================================================================
// u8 → u16 widening (Mode 6 convenience)
// ============================================================================

/// Widen a `u8` slice to `u16` for writing as Mode 6 (Uint16).
///
/// This matches Python `mrcfile`'s behaviour when given `np.uint8` data:
/// the data is automatically widened to `uint16` (mode 6) because MRC-2014
/// does not define a native unsigned 8-bit mode.
pub fn convert_u8_slice_to_u16(src: &[u8]) -> Vec<u16> {
    src.iter().map(|&v| v as u16).collect()
}

/// Narrow a `u16` slice to `u8`, returning `Err` if any value exceeds 255.
///
/// This is the reverse of [`convert_u8_slice_to_u16`] and is used when
/// reading a Mode 6 file that was originally created from `u8` data.
///
/// # Errors
/// Returns [`Error::ValueOutOfRange`](crate::Error::ValueOutOfRange) if any value exceeds 255.
pub fn convert_u16_slice_to_u8(src: &[u16]) -> Result<Vec<u8>, crate::Error> {
    let mut out = Vec::with_capacity(src.len());
    for &v in src {
        if v > 255 {
            return Err(crate::Error::ValueOutOfRange {
                value: v as u64,
                max: 255,
            });
        }
        out.push(v as u8);
    }
    Ok(out)
}

#[cfg(test)]
mod u8_tests {
    use super::*;

    #[test]
    fn test_convert_u8_to_u16() {
        let src: Vec<u8> = vec![0, 1, 127, 128, 255];
        let dst = convert_u8_slice_to_u16(&src);
        assert_eq!(dst, vec![0u16, 1, 127, 128, 255]);
    }

    #[test]
    fn test_convert_u16_to_u8_ok() {
        let src: Vec<u16> = vec![0, 1, 127, 128, 255];
        let dst = convert_u16_slice_to_u8(&src).unwrap();
        assert_eq!(dst, vec![0u8, 1, 127, 128, 255]);
    }

    #[test]
    fn test_convert_u16_to_u8_overflow() {
        let src: Vec<u16> = vec![0, 256];
        assert!(convert_u16_slice_to_u8(&src).is_err());
    }

    #[test]
    fn test_u8_roundtrip() {
        let original: Vec<u8> = (0..=255).collect();
        let widened = convert_u8_slice_to_u16(&original);
        let narrowed = convert_u16_slice_to_u8(&widened).unwrap();
        assert_eq!(original, narrowed);
    }
}
