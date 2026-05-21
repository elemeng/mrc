//! MRC-specific type conversions.
//!
//! This module provides conversions for the overwhelmingly common cryo-EM
//! workflows that the crate supports as conveniences:
//!
//! - `i8`/`i16`/`u16` → `f32` (for `slices_f32` / `slabs_f32`)
//! - Mode 0 reinterpretation (signed vs unsigned `i8`)
//! - 4-bit packed data unpacking
//!
//! The remaining conversions are `pub(crate)`; only the public free functions
//! in `lib.rs` are exposed.

use crate::mode::M0Interpretation;
use std::vec::Vec;

#[cfg(feature = "simd")]
use super::simd;

// === Packed4Bit (M101) Unpacking ===

/// Unpack raw 4-bit packed bytes to `u16`.
///
/// Each byte contains two nibbles. `num_values` specifies exactly how
/// many nibbles to extract, which is required when row widths are odd and
/// padding nibbles are present.
pub(crate) fn unpack_u4_bytes_to_u16(src: &[u8], num_values: usize) -> Vec<u16> {
    let mut dst = Vec::with_capacity(num_values);
    for &byte in src {
        dst.push((byte & 0x0F) as u16);
        if dst.len() >= num_values {
            break;
        }
        dst.push(((byte >> 4) & 0x0F) as u16);
        if dst.len() >= num_values {
            break;
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComplexToRealStrategy;
    use std::vec;

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
    fn test_unpack_u4_bytes_to_u16() {
        let bytes = vec![0x21, 0x43];
        let result = unpack_u4_bytes_to_u16(&bytes, 4);
        assert_eq!(result, vec![1, 2, 3, 4]);
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
