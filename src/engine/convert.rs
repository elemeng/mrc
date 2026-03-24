//! Type Conversion Layer (Layer 4 of the pipeline)
//!
//! This module implements voxel type conversions:
//! ```text
//! Typed voxel values → Converted voxel values
//! ```
//!
//! Conversions include:
//! - Int8 → Float32
//! - Int16 → Float32
//! - Uint16 → Float32
//! - Float16 → Float32
//! - Float32 → Int16
//! - Int16Complex ↔ Float32Complex
//! - Packed4Bit → all integer types
//! - u8 → all types

use crate::mode::{Float32Complex, Int16Complex, Packed4Bit};
use alloc::vec::Vec;

#[cfg(feature = "simd")]
use super::simd;

/// Trait for converting between voxel types.
///
/// This enables the type-level conversion graph described in engine.md.
pub trait Convert<S>: Sized {
    /// Convert a source value to the destination type
    fn convert(src: S) -> Self;
}

/// Attempt SIMD-accelerated conversion from source slice to destination type.
/// 
/// Returns Some(Vec<D>) if SIMD conversion is available, None otherwise.
/// Currently supports: i8/i16/u16/u8 → f32
#[cfg(feature = "simd")]
pub fn try_simd_convert<S: 'static, D: 'static>(src: &[S]) -> Option<Vec<D>>
where
    D: Convert<S> + 'static,
    S: 'static,
{
    use core::any::TypeId;
    
    // Check if destination is f32
    if TypeId::of::<D>() != TypeId::of::<f32>() {
        return None;
    }
    
    // Try each source type
    let result_f32: Vec<f32> = if TypeId::of::<S>() == TypeId::of::<i8>() {
        simd::convert_i8_to_f32_simd(unsafe { core::slice::from_raw_parts(src.as_ptr() as *const i8, src.len()) })
    } else if TypeId::of::<S>() == TypeId::of::<i16>() {
        simd::convert_i16_to_f32_simd(unsafe { core::slice::from_raw_parts(src.as_ptr() as *const i16, src.len()) })
    } else if TypeId::of::<S>() == TypeId::of::<u16>() {
        simd::convert_u16_to_f32_simd(unsafe { core::slice::from_raw_parts(src.as_ptr() as *const u16, src.len()) })
    } else if TypeId::of::<S>() == TypeId::of::<u8>() {
        simd::convert_u8_to_f32_simd(unsafe { core::slice::from_raw_parts(src.as_ptr() as *const u8, src.len()) })
    } else {
        return None;
    };
    
    // Safety: we verified D is f32, and result_f32 is Vec<f32>
    // Vec<f32> and Vec<D> have same layout when D is f32
    Some(unsafe { core::mem::transmute::<Vec<f32>, Vec<D>>(result_f32) })
}

/// Fallback when SIMD is disabled
#[cfg(not(feature = "simd"))]
pub fn try_simd_convert<S, D: Convert<S>>(_src: &[S]) -> Option<Vec<D>> {
    None
}

/// Attempt SIMD-accelerated conversion from f32 to integer types (write path).
///
/// Returns Some(Vec<D>) if SIMD conversion is available, None otherwise.
/// Currently supports: f32 → i8/i16/u16/u8
#[cfg(feature = "simd")]
pub fn try_simd_convert_reverse<S: 'static, D: 'static>(src: &[S]) -> Option<Vec<D>>
where
    S: 'static,
    D: Convert<S> + 'static,
{
    use core::any::TypeId;

    // Check if source is f32
    if TypeId::of::<S>() != TypeId::of::<f32>() {
        return None;
    }

    let _src_f32: &[f32] = unsafe { core::slice::from_raw_parts(src.as_ptr() as *const f32, src.len()) };

    // For write path, we use scalar conversion since SIMD conversion with clamping
    // is complex. The main benefit is on the read path (i16/u16 → f32).
    // Future: Add AVX-512 or NEON saturating conversion instructions.
    None
}

/// Fallback when SIMD is disabled
#[cfg(not(feature = "simd"))]
pub fn try_simd_convert_reverse<S, D: Convert<S>>(_src: &[S]) -> Option<Vec<D>> {
    None
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
pub fn convert_i8_slice_to_f32(src: &[i8]) -> Vec<f32> {
    simd::convert_i8_to_f32_simd(src)
}

/// Batch conversion from i8 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub fn convert_i8_slice_to_f32(src: &[i8]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from i16 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub fn convert_i16_slice_to_f32(src: &[i16]) -> Vec<f32> {
    simd::convert_i16_to_f32_simd(src)
}

/// Batch conversion from i16 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub fn convert_i16_slice_to_f32(src: &[i16]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from u16 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub fn convert_u16_slice_to_f32(src: &[u16]) -> Vec<f32> {
    simd::convert_u16_to_f32_simd(src)
}

/// Batch conversion from u16 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub fn convert_u16_slice_to_f32(src: &[u16]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from u8 to f32 using SIMD when available.
#[cfg(feature = "simd")]
pub fn convert_u8_slice_to_f32(src: &[u8]) -> Vec<f32> {
    simd::convert_u8_to_f32_simd(src)
}

/// Batch conversion from u8 to f32 (scalar fallback).
#[cfg(not(feature = "simd"))]
pub fn convert_u8_slice_to_f32(src: &[u8]) -> Vec<f32> {
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from f16 to f32.
#[cfg(feature = "f16")]
pub fn convert_f16_slice_to_f32(src: &[f16]) -> Vec<f32> {
    // Note: AVX-512 FP16 would provide hardware acceleration but requires
    // very recent CPUs. For now, use scalar conversion which is fast enough
    // for typical cryo-EM intermediate files.
    src.iter().map(|&x| x as f32).collect()
}

/// Batch conversion from f32 to f16.
#[cfg(feature = "f16")]
pub fn convert_f32_slice_to_f16(src: &[f32]) -> Vec<f16> {
    src.iter().map(|&x| x as f16).collect()
}

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
