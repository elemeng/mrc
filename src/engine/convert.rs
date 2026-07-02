//! MRC-specific type conversions.
//!
//! This module provides the generic conversion trait [`ConvertFrom`] that
//! powers the unified reader conversion system.
//!
//! Specific conversions:
//! - `i8`/`i16`/`u16` → `f32` (for `slices_f32` / `slabs_f32`)
//! - `f16` → `f32` (for `slices_f32`)
//! - `u8` → `u16`, `u16` → `u8` (Mode 6 utilities)
//! - Mode 0 reinterpretation (signed vs unsigned `i8`)
//! - 4-bit packed data unpacking/packing

use crate::Voxel;
use crate::mode::M0Interpretation;
use std::vec::Vec;

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
        src.iter().map(|&v| f32::from(v)).collect()
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
        M0Interpretation::Signed => data.iter().map(|&x| x as i8 as f32).collect(),
        M0Interpretation::Unsigned => data.iter().map(|&x| x as f32).collect(),
    }
}

// === Batch slice conversions (used by Reader::slices_f32 / slabs_f32) ===

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

// ============================================================================
// Generic conversion dispatcher — single match over all source modes
// ============================================================================

/// Decode raw bytes as source type `Src` and convert to destination type `Dst`
/// via the [`ConvertFrom`] trait.
pub(crate) fn convert_with<Src: Voxel, Dst>(
    bytes: &[u8],
    endian: FileEndian,
) -> Result<Vec<Dst>, Error>
where
    Dst: ConvertFrom<Src>,
{
    let src = decode_slice::<Src>(bytes, endian)?;
    Ok(Dst::convert_from(&src))
}

/// Convert a raw byte slice from any MRC mode to target type `T`.
///
/// This is the single dispatch point for all reader-side conversions.
/// The source mode is determined at runtime (from the file's header);
/// the target type `T` is a compile-time generic.
///
/// Handles all real-valued modes, complex modes (via magnitude), and
/// Packed4Bit (via nibble unpack).
#[cfg(feature = "f16")]
pub(crate) fn convert_block<T>(
    bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
    nx: usize,
    ny: usize,
) -> Result<Vec<T>, Error>
where
    T: Voxel + ConvertFrom<i8> + ConvertFrom<i16> + ConvertFrom<u16> + ConvertFrom<f32>,
{
    match mode {
        Mode::Int8 => convert_with::<i8, T>(bytes, endian),
        Mode::Int16 => convert_with::<i16, T>(bytes, endian),
        Mode::Uint16 => convert_with::<u16, T>(bytes, endian),
        Mode::Float32 => convert_with::<f32, T>(bytes, endian),
        Mode::Float16 => {
            // Route through f32 to avoid requiring T: ConvertFrom<crate::f16>
            let src = decode_slice::<crate::f16>(bytes, endian)?;
            let f32_data: Vec<f32> = src.iter().map(|&v| f32::from(v)).collect();
            Ok(T::convert_from(&f32_data))
        }
        Mode::Float32Complex => {
            let src = decode_slice::<Float32Complex>(bytes, endian)?;
            let mag: Vec<f32> = src
                .iter()
                .map(|c| c.to_real(ComplexToRealStrategy::Magnitude))
                .collect();
            Ok(T::convert_from(&mag))
        }
        Mode::Int16Complex => {
            let src = decode_slice::<Int16Complex>(bytes, endian)?;
            let mag: Vec<f32> = src
                .iter()
                .map(|c| c.to_real(ComplexToRealStrategy::Magnitude))
                .collect();
            Ok(T::convert_from(&mag))
        }
        Mode::Packed4Bit => {
            let unpacked = unpack_u4_bytes_to_u8(bytes, nx, ny);
            let f32_data: Vec<f32> = unpacked.iter().map(|&v| v as f32).collect();
            Ok(T::convert_from(&f32_data))
        }
    }
}

/// Convert a raw byte slice from any MRC mode to target type `T`.
///
/// This is the single dispatch point for all reader-side conversions.
/// The source mode is determined at runtime (from the file's header);
/// the target type `T` is a compile-time generic.
///
/// Handles all real-valued modes, complex modes (via magnitude), and
/// Packed4Bit (via nibble unpack).
#[cfg(not(feature = "f16"))]
pub(crate) fn convert_block<T>(
    bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
    nx: usize,
    ny: usize,
) -> Result<Vec<T>, Error>
where
    T: Voxel + ConvertFrom<i8> + ConvertFrom<i16> + ConvertFrom<u16> + ConvertFrom<f32>,
{
    match mode {
        Mode::Int8 => convert_with::<i8, T>(bytes, endian),
        Mode::Int16 => convert_with::<i16, T>(bytes, endian),
        Mode::Uint16 => convert_with::<u16, T>(bytes, endian),
        Mode::Float32 => convert_with::<f32, T>(bytes, endian),
        Mode::Float16 => Err(Error::UnsupportedMode),
        Mode::Float32Complex => {
            let src = decode_slice::<Float32Complex>(bytes, endian)?;
            let mag: Vec<f32> = src
                .iter()
                .map(|c| c.to_real(ComplexToRealStrategy::Magnitude))
                .collect();
            Ok(T::convert_from(&mag))
        }
        Mode::Int16Complex => {
            let src = decode_slice::<Int16Complex>(bytes, endian)?;
            let mag: Vec<f32> = src
                .iter()
                .map(|c| c.to_real(ComplexToRealStrategy::Magnitude))
                .collect();
            Ok(T::convert_from(&mag))
        }
        Mode::Packed4Bit => {
            let unpacked = unpack_u4_bytes_to_u8(bytes, nx, ny);
            let f32_data: Vec<f32> = unpacked.iter().map(|&v| v as f32).collect();
            Ok(T::convert_from(&f32_data))
        }
    }
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
        assert!((phase - 0.9272952).abs() < 1e-6);
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
pub fn convert_u16_slice_to_u8(src: &[u16]) -> Result<Vec<u8>, crate::Error> {
    let mut out = Vec::with_capacity(src.len());
    for &v in src {
        if v > 255 {
            return Err(crate::Error::TypeMismatch {
                expected: 1,
                actual: 2,
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
