//! MRC-specific type conversions.
//!
//! This module provides conversions for the overwhelmingly common cryo-EM
//! workflows that the crate supports as conveniences:
//!
//! - `i8`/`i16`/`u16` → `f32` (for `slices_f32` / `slabs_f32`)
//! - `f32` → `f16` (for `write_f16_from_f32`)
//! - Mode 0 reinterpretation (signed vs unsigned `i8`)
//! - 4-bit packed data unpacking
//!
//! Generic type conversion (`Convert` trait matrix) was intentionally removed
//! to keep the crate focused on MRC I/O. The remaining conversions are
//! `pub(crate)`; only the public free functions in `lib.rs` are exposed.

use crate::mode::{Float32Complex, Int16Complex, M0Interpretation, Packed4Bit};
use std::vec::Vec;

#[cfg(feature = "simd")]
use super::simd;

/// Trait for converting between voxel types.
///
/// This enables the type-level conversion graph described in engine.md.
pub(crate) trait Convert<S>: Sized {
    /// Convert a source value to the destination type
    fn convert(src: S) -> Self;
}

// === Packed4Bit (M101) Unpacking ===

/// Unpack 4-bit packed data to `u16`.
///
/// Each `Packed4Bit` contains two nibbles. `num_values` specifies exactly how
/// many nibbles to extract, which is required when row widths are odd and
/// padding nibbles are present.
pub fn unpack_u4_to_u16(src: &[Packed4Bit], num_values: usize) -> Vec<u16> {
    let mut dst = Vec::with_capacity(num_values);
    for packed in src {
        dst.push(packed.first() as u16);
        if dst.len() >= num_values {
            break;
        }
        dst.push(packed.second() as u16);
        if dst.len() >= num_values {
            break;
        }
    }
    dst
}

/// Unpack 4-bit packed data to `f32`.
pub fn unpack_u4_to_f32(src: &[Packed4Bit], num_values: usize) -> Vec<f32> {
    let mut dst = Vec::with_capacity(num_values);
    for packed in src {
        dst.push(packed.first() as f32);
        if dst.len() >= num_values {
            break;
        }
        dst.push(packed.second() as f32);
        if dst.len() >= num_values {
            break;
        }
    }
    dst
}

/// Unpack raw 4-bit packed bytes to `u16`.
pub fn unpack_u4_bytes_to_u16(src: &[u8], num_values: usize) -> Vec<u16> {
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

/// Unpack raw 4-bit packed bytes to `f32`.
pub fn unpack_u4_bytes_to_f32(src: &[u8], num_values: usize) -> Vec<f32> {
    let mut dst = Vec::with_capacity(num_values);
    for &byte in src {
        dst.push((byte & 0x0F) as f32);
        if dst.len() >= num_values {
            break;
        }
        dst.push(((byte >> 4) & 0x0F) as f32);
        if dst.len() >= num_values {
            break;
        }
    }
    dst
}

/// Batch unpack 4-bit packed data to `i8`.
pub fn unpack_u4_to_i8(src: &[Packed4Bit], num_values: usize) -> Vec<i8> {
    let mut dst = Vec::with_capacity(num_values);
    for packed in src {
        dst.push(i8::convert(*packed));
        if dst.len() >= num_values {
            break;
        }
        let second = Packed4Bit::new(packed.0).second();
        let signed = if second & 0x08 != 0 {
            (second | 0xF0) as i8
        } else {
            second as i8
        };
        dst.push(signed);
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

// === Conversions TO Float32 ===

impl Convert<i8> for f32 {
    #[inline]
    fn convert(src: i8) -> Self {
        src as f32
    }
}

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

/// Batch conversion from f16 to f32.
impl Convert<u8> for f32 {
    #[inline]
    fn convert(src: u8) -> Self {
        src as f32
    }
}

impl Convert<i16> for f32 {
    #[inline]
    fn convert(src: i16) -> Self {
        src as f32
    }
}

impl Convert<u16> for f32 {
    #[inline]
    fn convert(src: u16) -> Self {
        src as f32
    }
}

#[cfg(feature = "f16")]
impl Convert<f16> for f32 {
    #[inline]
    fn convert(src: f16) -> Self {
        src as f32
    }
}

impl Convert<f32> for f32 {
    #[inline]
    fn convert(src: f32) -> Self {
        src
    }
}

// === Conversions FROM Float32 ===

impl Convert<f32> for i8 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(i8::MIN as f32, i8::MAX as f32) as i8
    }
}

impl Convert<f32> for u8 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(u8::MIN as f32, u8::MAX as f32) as u8
    }
}

impl Convert<f32> for i16 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(i16::MIN as f32, i16::MAX as f32) as i16
    }
}

impl Convert<f32> for u16 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(u16::MIN as f32, u16::MAX as f32) as u16
    }
}

#[cfg(feature = "f16")]
impl Convert<f32> for f16 {
    #[inline]
    fn convert(src: f32) -> Self {
        src as f16
    }
}

// === Identity conversions ===

impl Convert<i8> for i8 {
    #[inline]
    fn convert(src: i8) -> Self {
        src
    }
}

impl Convert<i16> for i16 {
    #[inline]
    fn convert(src: i16) -> Self {
        src
    }
}

impl Convert<u16> for u16 {
    #[inline]
    fn convert(src: u16) -> Self {
        src
    }
}

// === Complex type conversions ===

impl Convert<Int16Complex> for Float32Complex {
    #[inline]
    fn convert(src: Int16Complex) -> Self {
        Self {
            real: src.real as f32,
            imag: src.imag as f32,
        }
    }
}

impl Convert<Float32Complex> for Int16Complex {
    #[inline]
    fn convert(src: Float32Complex) -> Self {
        Self {
            real: src.real.clamp(i16::MIN as f32, i16::MAX as f32) as i16,
            imag: src.imag.clamp(i16::MIN as f32, i16::MAX as f32) as i16,
        }
    }
}

impl Convert<Int16Complex> for Int16Complex {
    #[inline]
    fn convert(src: Int16Complex) -> Self {
        src
    }
}

impl Convert<Float32Complex> for Float32Complex {
    #[inline]
    fn convert(src: Float32Complex) -> Self {
        src
    }
}

// === Integer-to-Integer Conversions ===

impl Convert<i8> for i16 {
    #[inline]
    fn convert(src: i8) -> Self {
        src as i16
    }
}

impl Convert<i16> for i8 {
    #[inline]
    fn convert(src: i16) -> Self {
        src.clamp(i8::MIN as i16, i8::MAX as i16) as i8
    }
}

impl Convert<u8> for i16 {
    #[inline]
    fn convert(src: u8) -> Self {
        src as i16
    }
}

impl Convert<i16> for u8 {
    #[inline]
    fn convert(src: i16) -> Self {
        src.clamp(u8::MIN as i16, u8::MAX as i16) as u8
    }
}

impl Convert<u8> for u16 {
    #[inline]
    fn convert(src: u8) -> Self {
        src as u16
    }
}

impl Convert<u16> for u8 {
    #[inline]
    fn convert(src: u16) -> Self {
        src.clamp(u8::MIN as u16, u8::MAX as u16) as u8
    }
}

impl Convert<i8> for u16 {
    #[inline]
    fn convert(src: i8) -> Self {
        src.max(0) as u16
    }
}

impl Convert<u16> for i8 {
    #[inline]
    fn convert(src: u16) -> Self {
        (src as i16).clamp(i8::MIN as i16, i8::MAX as i16) as i8
    }
}

impl Convert<i16> for u16 {
    #[inline]
    fn convert(src: i16) -> Self {
        src.max(0) as u16
    }
}

impl Convert<u16> for i16 {
    #[inline]
    fn convert(src: u16) -> Self {
        src.min(i16::MAX as u16) as i16
    }
}

// === Packed4Bit Conversions ===

impl Convert<Packed4Bit> for u8 {
    #[inline]
    fn convert(src: Packed4Bit) -> Self {
        src.first()
    }
}

impl Convert<Packed4Bit> for i8 {
    #[inline]
    fn convert(src: Packed4Bit) -> Self {
        // Treat as signed: 0-7 positive, 8-15 negative (two's complement)
        let val = src.first();
        if val & 0x08 != 0 {
            (val | 0xF0) as i8 // Sign extend
        } else {
            val as i8
        }
    }
}

impl Convert<Packed4Bit> for f32 {
    #[inline]
    fn convert(src: Packed4Bit) -> Self {
        // Treat as unsigned 4-bit value
        src.first() as f32
    }
}

// === Identity Conversions (completing the matrix) ===

impl Convert<u8> for u8 {
    #[inline]
    fn convert(src: u8) -> Self {
        src
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use crate::ComplexToRealStrategy;

    // Test the Convert trait implementations
    #[test]
    fn test_convert_i8_to_f32_scalar() {
        assert_eq!(f32::convert(0i8), 0.0f32);
        assert_eq!(f32::convert(127i8), 127.0f32);
        assert_eq!(f32::convert(-128i8), -128.0f32);
        assert_eq!(f32::convert(42i8), 42.0f32);
    }

    #[test]
    fn test_convert_i16_to_f32_scalar() {
        assert_eq!(f32::convert(0i16), 0.0f32);
        assert_eq!(f32::convert(32767i16), 32767.0f32);
        assert_eq!(f32::convert(-32768i16), -32768.0f32);
        assert_eq!(f32::convert(1000i16), 1000.0f32);
    }

    #[test]
    fn test_convert_u16_to_f32_scalar() {
        assert_eq!(f32::convert(0u16), 0.0f32);
        assert_eq!(f32::convert(65535u16), 65535.0f32);
        assert_eq!(f32::convert(1000u16), 1000.0f32);
    }

    #[test]
    fn test_convert_u8_to_f32_scalar() {
        assert_eq!(f32::convert(0u8), 0.0f32);
        assert_eq!(f32::convert(255u8), 255.0f32);
        assert_eq!(f32::convert(128u8), 128.0f32);
    }

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
        let scalar_result: Vec<f32> = input.iter().map(|&x| f32::convert(x)).collect();
        assert_eq!(simd_result, scalar_result);
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_simd_scalar_equivalence_i16() {
        let input: Vec<i16> = (-32768..=-31768).collect(); // Full i16 range would be slow
        let simd_result = crate::engine::convert::convert_i16_slice_to_f32(&input);
        let scalar_result: Vec<f32> = input.iter().map(|&x| f32::convert(x)).collect();
        assert_eq!(simd_result, scalar_result);
    }

    #[test]
    #[cfg(feature = "simd")]
    fn test_simd_scalar_equivalence_u16() {
        let input: Vec<u16> = (0..10000).collect();
        let simd_result = crate::engine::convert::convert_u16_slice_to_f32(&input);
        let scalar_result: Vec<f32> = input.iter().map(|&x| f32::convert(x)).collect();
        assert_eq!(simd_result, scalar_result);
    }

    // Test M101 unpacking
    #[test]
    fn test_unpack_u4_to_u16_basic() {
        let packed = vec![Packed4Bit::new(0x21), Packed4Bit::new(0x43)];
        let result = unpack_u4_to_u16(&packed, 4);
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_unpack_u4_to_u16_odd_width() {
        // Odd width: 3 values from 2 bytes, ignoring padding nibble
        let packed = vec![Packed4Bit::new(0x21), Packed4Bit::new(0x43)];
        let result = unpack_u4_to_u16(&packed, 3);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_unpack_u4_bytes_to_u16() {
        let bytes = vec![0x21, 0x43];
        let result = unpack_u4_bytes_to_u16(&bytes, 4);
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_unpack_u4_to_f32() {
        let packed = vec![Packed4Bit::new(0x10), Packed4Bit::new(0x32)];
        let result = unpack_u4_to_f32(&packed, 4);
        assert_eq!(result, vec![0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_unpack_u4_bytes_to_f32() {
        let bytes = vec![0x10, 0x32];
        let result = unpack_u4_bytes_to_f32(&bytes, 4);
        assert_eq!(result, vec![0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_unpack_u4_to_i8() {
        // 0xF8 -> first nibble = 8 (signed: -8), second nibble = 15 (signed: -1)
        let packed = vec![Packed4Bit::new(0xF8)];
        let result = unpack_u4_to_i8(&packed, 2);
        assert_eq!(result, vec![-8i8, -1i8]);
    }

    #[test]
    fn test_unpack_u4_empty() {
        let packed: Vec<Packed4Bit> = vec![];
        let result = unpack_u4_to_u16(&packed, 0);
        assert!(result.is_empty());
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
        let c = Float32Complex { real: 3.0, imag: 4.0 };
        assert_eq!(c.to_real(ComplexToRealStrategy::RealPart), 3.0);
        assert_eq!(c.to_real(ComplexToRealStrategy::ImaginaryPart), 4.0);
        assert_eq!(c.to_real(ComplexToRealStrategy::Magnitude), 5.0);
        let phase = c.to_real(ComplexToRealStrategy::Phase);
        assert!((phase - 0.9272952).abs() < 1e-6);
    }
}
