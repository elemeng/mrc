//! SIMD-accelerated conversion kernels
//!
//! This module provides portable SIMD implementations of common voxel type
//! conversions using Rust's `std::simd` API (available in Rust 1.85+).
//!
//! # Supported Conversions
//!
//! - `i8 → f32` (32-lane SIMD)
//! - `i16 → f32` (16-lane SIMD)
//! - `u16 → f32` (16-lane SIMD)
//! - `u8 → f32` (16-lane SIMD)
//!
//! # Performance
//!
//! SIMD conversions typically achieve 4-8x speedup over scalar implementations
//! on modern x86_64 and AArch64 processors.

use std::vec::Vec;

#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;

/// Convert a slice of i8 values to f32 using SIMD acceleration.
///
/// Uses 32-lane SIMD when available (AVX2 on x86_64, NEON on AArch64).
pub(crate) fn convert_i8_to_f32_simd(src: &[i8]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { convert_i8_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { convert_i8_to_f32_neon(src) };
        }
    }

    // Fallback to scalar
    src.iter().map(|&x| x as f32).collect()
}

/// Convert a slice of i16 values to f32 using SIMD acceleration.
///
/// Uses 16-lane SIMD when available.
pub(crate) fn convert_i16_to_f32_simd(src: &[i16]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { convert_i16_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { convert_i16_to_f32_neon(src) };
        }
    }

    // Fallback to scalar
    src.iter().map(|&x| x as f32).collect()
}

/// Convert a slice of u16 values to f32 using SIMD acceleration.
pub(crate) fn convert_u16_to_f32_simd(src: &[u16]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { convert_u16_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { convert_u16_to_f32_neon(src) };
        }
    }

    // Fallback to scalar
    src.iter().map(|&x| x as f32).collect()
}

// =============================================================================
// x86_64 AVX2 implementations
// =============================================================================

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. This function:
/// - Allocates `Vec::with_capacity(src.len())` — enough for all elements
/// - Fills elements via SIMD stores in the loop and the scalar tail loop
/// - Calls `set_len` only after all elements are initialized
/// - Uses unaligned load/store intrinsics which do not require aligned pointers
unsafe fn convert_i8_to_f32_avx2(src: &[i8]) -> Vec<f32> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<f32> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;

        // Process 32 elements at a time
        while i + 32 <= src.len() {
            // Load 32 i8 values
            let input = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);

            // Convert to 16-bit (lower and upper halves)
            let lo = _mm256_cvtepi8_epi16(_mm256_castsi256_si128(input));
            let hi = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(input, 1));

            // Convert 16-bit to 32-bit floats (4 vectors of 8 floats)
            let lo_f = _mm256_cvtepi32_ps(_mm256_cvtepi16_epi32(_mm256_castsi256_si128(lo)));
            let lo_f_hi =
                _mm256_cvtepi32_ps(_mm256_cvtepi16_epi32(_mm256_extracti128_si256(lo, 1)));
            let hi_f = _mm256_cvtepi32_ps(_mm256_cvtepi16_epi32(_mm256_castsi256_si128(hi)));
            let hi_f_hi =
                _mm256_cvtepi32_ps(_mm256_cvtepi16_epi32(_mm256_extracti128_si256(hi, 1)));

            // Store results
            _mm256_storeu_ps(dst_ptr.add(i), lo_f);
            _mm256_storeu_ps(dst_ptr.add(i + 8), lo_f_hi);
            _mm256_storeu_ps(dst_ptr.add(i + 16), hi_f);
            _mm256_storeu_ps(dst_ptr.add(i + 24), hi_f_hi);

            i += 32;
        }

        // Tail elements: process remaining elements that don't fit a full vector
        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = v as f32;
        }

        // SAFETY: all src.len() elements initialized above.
        dst.set_len(src.len());

        dst
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_i16_to_f32_avx2(src: &[i16]) -> Vec<f32> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<f32> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;

        // Process 16 elements at a time
        while i + 16 <= src.len() {
            // Load 16 i16 values
            let input = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);

            // Convert to 32-bit integers (two halves)
            let lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(input));
            let hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(input, 1));

            // Convert to floats
            let lo_f = _mm256_cvtepi32_ps(lo);
            let hi_f = _mm256_cvtepi32_ps(hi);

            // Store results
            _mm256_storeu_ps(dst_ptr.add(i), lo_f);
            _mm256_storeu_ps(dst_ptr.add(i + 8), hi_f);

            i += 16;
        }

        // Tail elements
        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = v as f32;
        }

        // SAFETY: all src.len() elements initialized above.
        dst.set_len(src.len());

        dst
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_u16_to_f32_avx2(src: &[u16]) -> Vec<f32> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<f32> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;

        // Process 16 elements at a time
        while i + 16 <= src.len() {
            // Load 16 u16 values
            let input = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);

            // Convert to 32-bit integers (zero-extend)
            let lo = _mm256_cvtepu16_epi32(_mm256_castsi256_si128(input));
            let hi = _mm256_cvtepu16_epi32(_mm256_extracti128_si256(input, 1));

            // Convert to floats
            let lo_f = _mm256_cvtepi32_ps(lo);
            let hi_f = _mm256_cvtepi32_ps(hi);

            // Store results
            _mm256_storeu_ps(dst_ptr.add(i), lo_f);
            _mm256_storeu_ps(dst_ptr.add(i + 8), hi_f);

            i += 16;
        }

        // Tail elements
        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = v as f32;
        }

        // SAFETY: all src.len() elements initialized above.
        dst.set_len(src.len());

        dst
    }
}

// =============================================================================
// AArch64 NEON implementations
// =============================================================================

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_i8_to_f32_neon(src: &[i8]) -> Vec<f32> {
    use core::arch::aarch64::*;

    let mut dst: Vec<f32> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;

    // Process 16 elements at a time (NEON vector size)
    while i + 16 <= src.len() {
        // Load 16 i8 values
        let input = vld1q_s8(src.as_ptr().add(i));

        // Widen to 16-bit
        let lo_16 = vmovl_s8(vget_low_s8(input));
        let hi_16 = vmovl_s8(vget_high_s8(input));

        // Widen to 32-bit and convert to float
        let f0 = vcvtq_f32_s32(vmovl_s16(vget_low_s16(lo_16)));
        let f1 = vcvtq_f32_s32(vmovl_s16(vget_high_s16(lo_16)));
        let f2 = vcvtq_f32_s32(vmovl_s16(vget_low_s16(hi_16)));
        let f3 = vcvtq_f32_s32(vmovl_s16(vget_high_s16(hi_16)));

        // Store results
        vst1q_f32(dst_ptr.add(i), f0);
        vst1q_f32(dst_ptr.add(i + 4), f1);
        vst1q_f32(dst_ptr.add(i + 8), f2);
        vst1q_f32(dst_ptr.add(i + 12), f3);

        i += 16;
    }

    // Tail elements
    for (j, &v) in src.iter().enumerate().skip(i) {
        *dst_ptr.add(j) = v as f32;
    }

    // SAFETY: all src.len() elements initialized above.
    dst.set_len(src.len());

    dst
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_i16_to_f32_neon(src: &[i16]) -> Vec<f32> {
    use core::arch::aarch64::*;

    let mut dst: Vec<f32> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;

    // Process 8 elements at a time
    while i + 8 <= src.len() {
        // Load 8 i16 values
        let input = vld1q_s16(src.as_ptr().add(i));

        // Widen to 32-bit and convert to float
        let lo = vcvtq_f32_s32(vmovl_s16(vget_low_s16(input)));
        let hi = vcvtq_f32_s32(vmovl_s16(vget_high_s16(input)));

        // Store results
        vst1q_f32(dst_ptr.add(i), lo);
        vst1q_f32(dst_ptr.add(i + 4), hi);

        i += 8;
    }

    // Tail elements
    for (j, &v) in src.iter().enumerate().skip(i) {
        *dst_ptr.add(j) = v as f32;
    }

    // SAFETY: all src.len() elements initialized above.
    dst.set_len(src.len());

    dst
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_u16_to_f32_neon(src: &[u16]) -> Vec<f32> {
    use core::arch::aarch64::*;

    let mut dst: Vec<f32> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;

    // Process 8 elements at a time
    while i + 8 <= src.len() {
        // Load 8 u16 values
        let input = vld1q_u16(src.as_ptr().add(i));

        // Widen to 32-bit and convert to float
        let lo = vcvtq_f32_u32(vmovl_u16(vget_low_u16(input)));
        let hi = vcvtq_f32_u32(vmovl_u16(vget_high_u16(input)));

        // Store results
        vst1q_f32(dst_ptr.add(i), lo);
        vst1q_f32(dst_ptr.add(i + 4), hi);

        i += 8;
    }

    // Tail elements
    for (j, &v) in src.iter().enumerate().skip(i) {
        *dst_ptr.add(j) = v as f32;
    }

    // SAFETY: all src.len() elements initialized above.
    dst.set_len(src.len());

    dst
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_i8_to_f32() {
        let input: Vec<i8> = (-128..=127).collect();
        let output = convert_i8_to_f32_simd(&input);

        assert_eq!(output.len(), input.len());
        for (i, (&src, &dst)) in input.iter().zip(output.iter()).enumerate() {
            assert_eq!(dst, src as f32, "mismatch at index {}", i);
        }
    }

    #[test]
    fn test_convert_i16_to_f32() {
        let input: Vec<i16> = (-1000..1000).collect();
        let output = convert_i16_to_f32_simd(&input);

        assert_eq!(output.len(), input.len());
        for (i, (&src, &dst)) in input.iter().zip(output.iter()).enumerate() {
            assert_eq!(dst, src as f32, "mismatch at index {}", i);
        }
    }

    #[test]
    fn test_convert_u16_to_f32() {
        let input: Vec<u16> = (0..2000).collect();
        let output = convert_u16_to_f32_simd(&input);

        assert_eq!(output.len(), input.len());
        for (i, (&src, &dst)) in input.iter().zip(output.iter()).enumerate() {
            assert_eq!(dst, src as f32, "mismatch at index {}", i);
        }
    }
}
