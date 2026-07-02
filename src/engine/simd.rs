//! SIMD-accelerated conversion kernels
//!
//! Provides platform-specific SIMD implementations of common voxel type
//! conversions using raw `core::arch` intrinsics (AVX2 on x86_64, NEON on
//! AArch64). All SIMD paths use **runtime feature detection** — they fall
//! back to scalar code when the required ISA is not available, so the binary
//! runs on any CPU of the target architecture.
//!
//! # Supported Conversions
//!
//! - `i8 → f32` (32-lane SIMD)
//! - `i16 → f32` (16-lane SIMD)
//! - `u16 → f32` (16-lane SIMD)
//! - `u8 → f32` (32-lane SIMD, for unsigned Mode 0)
//! - `f16 → f32` (16-lane SIMD via F16C / NEON fp16)
//! - `f32 → f16` (16-lane SIMD via F16C / NEON fp16)
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

/// Convert a slice of u8 values to f32 using SIMD acceleration.
///
/// Uses 32-lane SIMD when available (AVX2 on x86_64, NEON on AArch64).
/// This supports unsigned Mode 0 interpretation.
pub(crate) fn convert_u8_to_f32_simd(src: &[u8]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { convert_u8_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { convert_u8_to_f32_neon(src) };
        }
    }

    // Fallback to scalar
    src.iter().map(|&x| x as f32).collect()
}

/// Convert a slice of f16 values to f32 using SIMD acceleration.
///
/// Uses F16C on x86_64 or fp16 on AArch64 when available.
#[cfg(feature = "f16")]
pub(crate) fn convert_f16_to_f32_simd(src: &[crate::f16]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("f16c") {
            return unsafe { convert_f16_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("fp16") {
            return unsafe { convert_f16_to_f32_neon(src) };
        }
    }

    // Fallback to scalar
    src.iter().map(|&v| f32::from(v)).collect()
}

/// Convert a slice of f32 values to f16 using SIMD acceleration.
///
/// Uses F16C on x86_64 or fp16 on AArch64 when available.
#[cfg(feature = "f16")]
pub(crate) fn convert_f32_to_f16_simd(src: &[f32]) -> Vec<crate::f16> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("f16c") {
            return unsafe { convert_f32_to_f16_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("fp16") {
            return unsafe { convert_f32_to_f16_neon(src) };
        }
    }

    // Fallback to scalar
    src.iter().map(|&v| crate::f16::from_f32(v)).collect()
}

/// Compute (dmin, dmax, dmean, rms) for f32 data using SIMD acceleration.
///
/// Uses a two-pass approach: pass 1 computes min/max/sum via SIMD horizontal
/// reduction, pass 2 computes variance via SIMD FMA/subtract.
/// Falls back to scalar on unsupported hardware.
pub(crate) fn stats_f32_simd(data: &[f32]) -> (f32, f32, f32, f32) {
    if data.is_empty() {
        return (0.0, -1.0, -2.0, -1.0);
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { stats_f32_avx2(data) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { stats_f32_neon(data) };
        }
    }

    // Scalar fallback: single-pass Welford
    stats_f32_scalar(data)
}

/// Scalar single-pass f32 statistics (Welford's online algorithm).
fn stats_f32_scalar(data: &[f32]) -> (f32, f32, f32, f32) {
    let len = data.len();
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut n = 0u64;
    let mut mean = 0.0f64;
    let mut m2 = 0.0f64;

    for &v in data {
        let x = v as f64;
        n += 1;
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        let delta = x - mean;
        mean += delta / n as f64;
        m2 += delta * (x - mean);
    }

    let rms = (m2 / len as f64).sqrt() as f32;
    (min, max, mean as f32, rms)
}

/// Swap byte order within 2-byte groups. Supports i16/u16/f16 endian conversion.
pub(crate) fn swap_2byte_simd(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(src.len(), dst.len());
    debug_assert!(src.len() % 2 == 0);

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { swap_2byte_avx2(src, dst) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { swap_2byte_neon(src, dst) };
        }
    }

    // Fallback to scalar
    for (i, chunk) in src.chunks_exact(2).enumerate() {
        dst[i * 2] = chunk[1];
        dst[i * 2 + 1] = chunk[0];
    }
}

/// Swap byte order within 4-byte groups. Supports i32/f32/u32 endian conversion.
pub(crate) fn swap_4byte_simd(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(src.len(), dst.len());
    debug_assert!(src.len() % 4 == 0);

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { swap_4byte_avx2(src, dst) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { swap_4byte_neon(src, dst) };
        }
    }

    // Fallback to scalar
    for (i, chunk) in src.chunks_exact(4).enumerate() {
        dst[i * 4] = chunk[3];
        dst[i * 4 + 1] = chunk[2];
        dst[i * 4 + 2] = chunk[1];
        dst[i * 4 + 3] = chunk[0];
    }
}

/// Swap byte order within 8-byte groups. Supports f64/i64/u64 endian conversion.
pub(crate) fn swap_8byte_simd(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(src.len(), dst.len());
    debug_assert!(src.len() % 8 == 0);

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { swap_8byte_avx2(src, dst) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { swap_8byte_neon(src, dst) };
        }
    }

    // Fallback to scalar
    for (i, chunk) in src.chunks_exact(8).enumerate() {
        dst[i * 8] = chunk[7];
        dst[i * 8 + 1] = chunk[6];
        dst[i * 8 + 2] = chunk[5];
        dst[i * 8 + 3] = chunk[4];
        dst[i * 8 + 4] = chunk[3];
        dst[i * 8 + 5] = chunk[2];
        dst[i * 8 + 6] = chunk[1];
        dst[i * 8 + 7] = chunk[0];
    }
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

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_u8_to_f32_avx2(src: &[u8]) -> Vec<f32> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<f32> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;

        // Process 32 elements at a time
        while i + 32 <= src.len() {
            // Load 32 u8 values
            let input = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);

            // Zero-extend to 16-bit (lower and upper halves)
            let lo = _mm256_cvtepu8_epi16(_mm256_castsi256_si128(input));
            let hi = _mm256_cvtepu8_epi16(_mm256_extracti128_si256(input, 1));

            // Zero-extend to 32-bit and convert to float
            let lo_f = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_castsi256_si128(lo)));
            let lo_f_hi =
                _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_extracti128_si256(lo, 1)));
            let hi_f = _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_castsi256_si128(hi)));
            let hi_f_hi =
                _mm256_cvtepi32_ps(_mm256_cvtepu16_epi32(_mm256_extracti128_si256(hi, 1)));

            // Store results
            _mm256_storeu_ps(dst_ptr.add(i), lo_f);
            _mm256_storeu_ps(dst_ptr.add(i + 8), lo_f_hi);
            _mm256_storeu_ps(dst_ptr.add(i + 16), hi_f);
            _mm256_storeu_ps(dst_ptr.add(i + 24), hi_f_hi);

            i += 32;
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
#[target_feature(enable = "f16c")]
/// SAFETY: Caller must ensure F16C is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_f16_to_f32_avx2(src: &[crate::f16]) -> Vec<f32> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<f32> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let src_u16: &[u16] = core::slice::from_raw_parts(src.as_ptr() as *const u16, src.len());
        let mut i = 0;

        // Process 16 elements at a time (2 × _mm256_cvtph_ps)
        while i + 16 <= src.len() {
            // Load 8 f16 values as __m128i each, convert to __m256f
            let lo = _mm_loadu_si128(src_u16.as_ptr().add(i) as *const __m128i);
            let hi = _mm_loadu_si128(src_u16.as_ptr().add(i + 8) as *const __m128i);

            let f_lo = _mm256_cvtph_ps(lo);
            let f_hi = _mm256_cvtph_ps(hi);

            _mm256_storeu_ps(dst_ptr.add(i), f_lo);
            _mm256_storeu_ps(dst_ptr.add(i + 8), f_hi);

            i += 16;
        }

        // Tail elements
        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = f32::from(v);
        }

        // SAFETY: all src.len() elements initialized above.
        dst.set_len(src.len());

        dst
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "f16c")]
/// SAFETY: Caller must ensure F16C is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_f32_to_f16_avx2(src: &[f32]) -> Vec<crate::f16> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<crate::f16> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let dst_u16 = dst.as_mut_ptr() as *mut u16;
        let mut i = 0;

        // Process 16 elements at a time (2 × _mm256_cvtps_ph)
        while i + 16 <= src.len() {
            let f_lo = _mm256_loadu_ps(src.as_ptr().add(i));
            let f_hi = _mm256_loadu_ps(src.as_ptr().add(i + 8));

            let lo = _mm256_cvtps_ph(f_lo, _MM_FROUND_TO_NEAREST_INT);
            let hi = _mm256_cvtps_ph(f_hi, _MM_FROUND_TO_NEAREST_INT);

            _mm_storeu_si128(dst_u16.add(i) as *mut __m128i, lo);
            _mm_storeu_si128(dst_u16.add(i + 8) as *mut __m128i, hi);

            i += 16;
        }

        // Tail elements
        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = crate::f16::from_f32(v);
        }

        // SAFETY: all src.len() elements initialized above.
        dst.set_len(src.len());

        dst
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime.
/// Swaps every 2-byte pair using PSHUFB.
unsafe fn swap_2byte_avx2(src: &[u8], dst: &mut [u8]) {
    unsafe {
        use core::arch::x86_64::*;

        let mask = _mm256_setr_epi8(
            1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14, 1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11,
            10, 13, 12, 15, 14,
        );
        let mut i = 0;
        while i + 32 <= src.len() {
            let data = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);
            let swapped = _mm256_shuffle_epi8(data, mask);
            _mm256_storeu_si256(dst.as_mut_ptr().add(i) as *mut __m256i, swapped);
            i += 32;
        }
        // Tail
        for (j, chunk) in src[i..].chunks_exact(2).enumerate() {
            let idx = i + j * 2;
            dst[idx] = chunk[1];
            dst[idx + 1] = chunk[0];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime.
/// Swaps every 4-byte group using PSHUFB.
unsafe fn swap_4byte_avx2(src: &[u8], dst: &mut [u8]) {
    unsafe {
        use core::arch::x86_64::*;

        let mask = _mm256_setr_epi8(
            3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12, 3, 2, 1, 0, 7, 6, 5, 4, 11, 10,
            9, 8, 15, 14, 13, 12,
        );
        let mut i = 0;
        while i + 32 <= src.len() {
            let data = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);
            let swapped = _mm256_shuffle_epi8(data, mask);
            _mm256_storeu_si256(dst.as_mut_ptr().add(i) as *mut __m256i, swapped);
            i += 32;
        }
        // Tail
        for (j, chunk) in src[i..].chunks_exact(4).enumerate() {
            let idx = i + j * 4;
            dst[idx] = chunk[3];
            dst[idx + 1] = chunk[2];
            dst[idx + 2] = chunk[1];
            dst[idx + 3] = chunk[0];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime.
/// Swaps every 8-byte group using PSHUFB.
unsafe fn swap_8byte_avx2(src: &[u8], dst: &mut [u8]) {
    unsafe {
        use core::arch::x86_64::*;

        let mask = _mm256_setr_epi8(
            7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 15, 14,
            13, 12, 11, 10, 9, 8,
        );
        let mut i = 0;
        while i + 32 <= src.len() {
            let data = _mm256_loadu_si256(src.as_ptr().add(i) as *const __m256i);
            let swapped = _mm256_shuffle_epi8(data, mask);
            _mm256_storeu_si256(dst.as_mut_ptr().add(i) as *mut __m256i, swapped);
            i += 32;
        }
        // Tail
        for (j, chunk) in src[i..].chunks_exact(8).enumerate() {
            let idx = i + j * 8;
            dst[idx] = chunk[7];
            dst[idx + 1] = chunk[6];
            dst[idx + 2] = chunk[5];
            dst[idx + 3] = chunk[4];
            dst[idx + 4] = chunk[3];
            dst[idx + 5] = chunk[2];
            dst[idx + 6] = chunk[1];
            dst[idx + 7] = chunk[0];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available.
/// Two-pass SIMD f32 statistics: pass 1 = min/max/sum, pass 2 = variance.
unsafe fn stats_f32_avx2(data: &[f32]) -> (f32, f32, f32, f32) {
    unsafe {
        use core::arch::x86_64::*;

        let len = data.len();
        let mut i = 0;

        // Pass 1: min, max, sum
        let mut vmin = _mm256_set1_ps(f32::INFINITY);
        let mut vmax = _mm256_set1_ps(f32::NEG_INFINITY);
        let mut vsum = _mm256_setzero_ps();

        while i + 32 <= len {
            let d0 = _mm256_loadu_ps(data.as_ptr().add(i));
            let d1 = _mm256_loadu_ps(data.as_ptr().add(i + 8));
            let d2 = _mm256_loadu_ps(data.as_ptr().add(i + 16));
            let d3 = _mm256_loadu_ps(data.as_ptr().add(i + 24));
            vmin = _mm256_min_ps(
                vmin,
                _mm256_min_ps(d0, _mm256_min_ps(d1, _mm256_min_ps(d2, d3))),
            );
            vmax = _mm256_max_ps(
                vmax,
                _mm256_max_ps(d0, _mm256_max_ps(d1, _mm256_max_ps(d2, d3))),
            );
            vsum = _mm256_add_ps(
                vsum,
                _mm256_add_ps(d0, _mm256_add_ps(d1, _mm256_add_ps(d2, d3))),
            );
            i += 32;
        }
        while i + 8 <= len {
            let d = _mm256_loadu_ps(data.as_ptr().add(i));
            vmin = _mm256_min_ps(vmin, d);
            vmax = _mm256_max_ps(vmax, d);
            vsum = _mm256_add_ps(vsum, d);
            i += 8;
        }

        // Horizontal reduce
        let mut hmin = [f32::INFINITY; 8];
        let mut hmax = [f32::NEG_INFINITY; 8];
        let mut hsum = [0.0f32; 8];
        _mm256_storeu_ps(hmin.as_mut_ptr(), vmin);
        _mm256_storeu_ps(hmax.as_mut_ptr(), vmax);
        _mm256_storeu_ps(hsum.as_mut_ptr(), vsum);

        let mut min = hmin[0];
        let mut max = hmax[0];
        let mut sum = 0.0f64;
        for j in 0..8 {
            if hmin[j] < min {
                min = hmin[j];
            }
            if hmax[j] > max {
                max = hmax[j];
            }
            sum += hsum[j] as f64;
        }

        // Tail elements
        for &v in &data[i..] {
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
            sum += v as f64;
        }

        let mean = (sum / len as f64) as f32;

        // Pass 2: variance using SIMD
        let vmean = _mm256_set1_ps(mean);
        let mut vvar = _mm256_setzero_ps();
        let mut j = 0;
        while j + 32 <= len {
            let d0 = _mm256_loadu_ps(data.as_ptr().add(j));
            let d1 = _mm256_loadu_ps(data.as_ptr().add(j + 8));
            let d2 = _mm256_loadu_ps(data.as_ptr().add(j + 16));
            let d3 = _mm256_loadu_ps(data.as_ptr().add(j + 24));
            let s0 = _mm256_sub_ps(d0, vmean);
            let s1 = _mm256_sub_ps(d1, vmean);
            let s2 = _mm256_sub_ps(d2, vmean);
            let s3 = _mm256_sub_ps(d3, vmean);
            vvar = _mm256_add_ps(
                vvar,
                _mm256_add_ps(
                    _mm256_add_ps(_mm256_mul_ps(s0, s0), _mm256_mul_ps(s1, s1)),
                    _mm256_add_ps(_mm256_mul_ps(s2, s2), _mm256_mul_ps(s3, s3)),
                ),
            );
            j += 32;
        }
        while j + 8 <= len {
            let d = _mm256_loadu_ps(data.as_ptr().add(j));
            let s = _mm256_sub_ps(d, vmean);
            vvar = _mm256_add_ps(vvar, _mm256_mul_ps(s, s));
            j += 8;
        }

        let mut var_acc = [0.0f32; 8];
        _mm256_storeu_ps(var_acc.as_mut_ptr(), vvar);
        let mut variance = 0.0f64;
        for &v in &var_acc {
            variance += v as f64;
        }
        // Tail elements for variance
        for &v in &data[j..] {
            let d = v as f64 - mean as f64;
            variance += d * d;
        }

        let rms = (variance / len as f64).sqrt() as f32;
        (min, max, mean, rms)
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

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
unsafe fn convert_u8_to_f32_neon(src: &[u8]) -> Vec<f32> {
    use core::arch::aarch64::*;

    let mut dst: Vec<f32> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;

    // Process 16 elements at a time (NEON vector size)
    while i + 16 <= src.len() {
        // Load 16 u8 values
        let input = vld1q_u8(src.as_ptr().add(i));

        // Widen to 16-bit
        let lo_16 = vmovl_u8(vget_low_u8(input));
        let hi_16 = vmovl_u8(vget_high_u8(input));

        // Widen to 32-bit and convert to float (unsigned)
        let f0 = vcvtq_f32_u32(vmovl_u16(vget_low_u16(lo_16)));
        let f1 = vcvtq_f32_u32(vmovl_u16(vget_high_u16(lo_16)));
        let f2 = vcvtq_f32_u32(vmovl_u16(vget_low_u16(hi_16)));
        let f3 = vcvtq_f32_u32(vmovl_u16(vget_high_u16(hi_16)));

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
#[target_feature(enable = "fp16")]
/// SAFETY: Caller must ensure fp16 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
///
/// Uses `vcvt_f32_f16` to convert 4 half-precision floats per call,
/// processing 8 elements per loop iteration.
unsafe fn convert_f16_to_f32_neon(src: &[crate::f16]) -> Vec<f32> {
    use core::arch::aarch64::*;

    let mut dst: Vec<f32> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let src_u16: &[u16] = core::slice::from_raw_parts(src.as_ptr() as *const u16, src.len());
    let mut i = 0;

    // Process 8 elements at a time (2 × vcvt_f32_f16)
    while i + 8 <= src.len() {
        let lo = vld1_f16(src_u16.as_ptr().add(i) as *const float16_t);
        let hi = vld1_f16(src_u16.as_ptr().add(i + 4) as *const float16_t);

        let f_lo = vcvt_f32_f16(lo);
        let f_hi = vcvt_f32_f16(hi);

        vst1q_f32(dst_ptr.add(i), f_lo);
        vst1q_f32(dst_ptr.add(i + 4), f_hi);

        i += 8;
    }

    // Tail elements
    for (j, &v) in src.iter().enumerate().skip(i) {
        *dst_ptr.add(j) = f32::from(v);
    }

    // SAFETY: all src.len() elements initialized above.
    dst.set_len(src.len());

    dst
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "fp16")]
/// SAFETY: Caller must ensure fp16 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
///
/// Uses `vcvt_f16_f32` to convert 4 single-precision floats per call,
/// processing 8 elements per loop iteration.
unsafe fn convert_f32_to_f16_neon(src: &[f32]) -> Vec<crate::f16> {
    use core::arch::aarch64::*;

    let mut dst: Vec<crate::f16> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let dst_u16 = dst.as_mut_ptr() as *mut u16;
    let mut i = 0;

    // Process 8 elements at a time (2 × vcvt_f16_f32)
    while i + 8 <= src.len() {
        let f_lo = vld1q_f32(src.as_ptr().add(i));
        let f_hi = vld1q_f32(src.as_ptr().add(i + 4));

        let lo = vcvt_f16_f32(f_lo);
        let hi = vcvt_f16_f32(f_hi);

        vst1_f16(dst_u16.add(i) as *mut float16_t, lo);
        vst1_f16(dst_u16.add(i + 4) as *mut float16_t, hi);

        i += 8;
    }

    // Tail elements
    for (j, &v) in src.iter().enumerate().skip(i) {
        *dst_ptr.add(j) = crate::f16::from_f32(v);
    }

    // SAFETY: all src.len() elements initialized above.
    dst.set_len(src.len());

    dst
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime.
/// Swaps every 2-byte pair using vrev16q_u8.
unsafe fn swap_2byte_neon(src: &[u8], dst: &mut [u8]) {
    use core::arch::aarch64::*;
    let mut i = 0;
    while i + 16 <= src.len() {
        let data = vld1q_u8(src.as_ptr().add(i));
        let swapped = vrev16q_u8(data);
        vst1q_u8(dst.as_mut_ptr().add(i), swapped);
        i += 16;
    }
    // Tail
    for (j, chunk) in src[i..].chunks_exact(2).enumerate() {
        let idx = i + j * 2;
        dst[idx] = chunk[1];
        dst[idx + 1] = chunk[0];
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime.
/// Swaps every 4-byte group using vrev32q_u8.
unsafe fn swap_4byte_neon(src: &[u8], dst: &mut [u8]) {
    use core::arch::aarch64::*;
    let mut i = 0;
    while i + 16 <= src.len() {
        let data = vld1q_u8(src.as_ptr().add(i));
        let swapped = vrev32q_u8(data);
        vst1q_u8(dst.as_mut_ptr().add(i), swapped);
        i += 16;
    }
    // Tail
    for (j, chunk) in src[i..].chunks_exact(4).enumerate() {
        let idx = i + j * 4;
        dst[idx] = chunk[3];
        dst[idx + 1] = chunk[2];
        dst[idx + 2] = chunk[1];
        dst[idx + 3] = chunk[0];
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime.
/// Swaps every 8-byte group using vrev64q_u8.
unsafe fn swap_8byte_neon(src: &[u8], dst: &mut [u8]) {
    use core::arch::aarch64::*;
    let mut i = 0;
    while i + 16 <= src.len() {
        let data = vld1q_u8(src.as_ptr().add(i));
        let swapped = vrev64q_u8(data);
        vst1q_u8(dst.as_mut_ptr().add(i), swapped);
        i += 16;
    }
    // Tail
    for (j, chunk) in src[i..].chunks_exact(8).enumerate() {
        let idx = i + j * 8;
        dst[idx] = chunk[7];
        dst[idx + 1] = chunk[6];
        dst[idx + 2] = chunk[5];
        dst[idx + 3] = chunk[4];
        dst[idx + 4] = chunk[3];
        dst[idx + 5] = chunk[2];
        dst[idx + 6] = chunk[1];
        dst[idx + 7] = chunk[0];
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available.
/// Two-pass SIMD f32 statistics: pass 1 = min/max/sum, pass 2 = variance.
unsafe fn stats_f32_neon(data: &[f32]) -> (f32, f32, f32, f32) {
    use core::arch::aarch64::*;

    let len = data.len();
    let mut i = 0;

    // Pass 1: min, max, sum
    let mut vmin = vdupq_n_f32(f32::INFINITY);
    let mut vmax = vdupq_n_f32(f32::NEG_INFINITY);
    let mut vsum = vdupq_n_f32(0.0);

    while i + 16 <= len {
        let d0 = vld1q_f32(data.as_ptr().add(i));
        let d1 = vld1q_f32(data.as_ptr().add(i + 4));
        let d2 = vld1q_f32(data.as_ptr().add(i + 8));
        let d3 = vld1q_f32(data.as_ptr().add(i + 12));
        vmin = vminq_f32(vmin, vminq_f32(d0, vminq_f32(d1, vminq_f32(d2, d3))));
        vmax = vmaxq_f32(vmax, vmaxq_f32(d0, vmaxq_f32(d1, vmaxq_f32(d2, d3))));
        vsum = vaddq_f32(vsum, vaddq_f32(d0, vaddq_f32(d1, vaddq_f32(d2, d3))));
        i += 16;
    }
    while i + 4 <= len {
        let d = vld1q_f32(data.as_ptr().add(i));
        vmin = vminq_f32(vmin, d);
        vmax = vmaxq_f32(vmax, d);
        vsum = vaddq_f32(vsum, d);
        i += 4;
    }

    // Horizontal reduce
    let mut hmin = [f32::INFINITY; 4];
    let mut hmax = [f32::NEG_INFINITY; 4];
    let mut hsum = [0.0f32; 4];
    vst1q_f32(hmin.as_mut_ptr(), vmin);
    vst1q_f32(hmax.as_mut_ptr(), vmax);
    vst1q_f32(hsum.as_mut_ptr(), vsum);

    let mut min = hmin[0];
    let mut max = hmax[0];
    let mut sum = 0.0f64;
    for j in 0..4 {
        if hmin[j] < min {
            min = hmin[j];
        }
        if hmax[j] > max {
            max = hmax[j];
        }
        sum += hsum[j] as f64;
    }

    // Tail elements
    for &v in &data[i..] {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        sum += v as f64;
    }

    let mean = (sum / len as f64) as f32;

    // Pass 2: variance using SIMD
    let vmean = vdupq_n_f32(mean);
    let mut vvar = vdupq_n_f32(0.0);
    let mut j = 0;
    while j + 16 <= len {
        let d0 = vld1q_f32(data.as_ptr().add(j));
        let d1 = vld1q_f32(data.as_ptr().add(j + 4));
        let d2 = vld1q_f32(data.as_ptr().add(j + 8));
        let d3 = vld1q_f32(data.as_ptr().add(j + 12));
        let s0 = vsubq_f32(d0, vmean);
        let s1 = vsubq_f32(d1, vmean);
        let s2 = vsubq_f32(d2, vmean);
        let s3 = vsubq_f32(d3, vmean);
        vvar = vaddq_f32(
            vvar,
            vaddq_f32(
                vaddq_f32(vmulq_f32(s0, s0), vmulq_f32(s1, s1)),
                vaddq_f32(vmulq_f32(s2, s2), vmulq_f32(s3, s3)),
            ),
        );
        j += 16;
    }
    while j + 4 <= len {
        let d = vld1q_f32(data.as_ptr().add(j));
        let s = vsubq_f32(d, vmean);
        vvar = vaddq_f32(vvar, vmulq_f32(s, s));
        j += 4;
    }

    let mut var_acc = [0.0f32; 4];
    vst1q_f32(var_acc.as_mut_ptr(), vvar);
    let mut variance = 0.0f64;
    for &v in &var_acc {
        variance += v as f64;
    }
    // Tail elements for variance
    for &v in &data[j..] {
        let d = v as f64 - mean as f64;
        variance += d * d;
    }

    let rms = (variance / len as f64).sqrt() as f32;
    (min, max, mean, rms)
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

    #[test]
    fn test_convert_u8_to_f32() {
        let input: Vec<u8> = (0..255).collect();
        let output = convert_u8_to_f32_simd(&input);

        assert_eq!(output.len(), input.len());
        for (i, (&src, &dst)) in input.iter().zip(output.iter()).enumerate() {
            assert_eq!(dst, src as f32, "mismatch at index {}", i);
        }
    }

    #[cfg(feature = "f16")]
    #[test]
    fn test_convert_f16_to_f32() {
        let src_f32: Vec<f32> = (-128..128).map(|i| i as f32 * 0.125).collect();
        let input: Vec<crate::f16> = src_f32.iter().map(|&v| crate::f16::from_f32(v)).collect();
        let output = convert_f16_to_f32_simd(&input);

        assert_eq!(output.len(), input.len());
        for (i, (&expected, &got)) in src_f32.iter().zip(output.iter()).enumerate() {
            let diff = (expected - got).abs();
            // f16 has ~3.3 decimal digits of precision; allow 0.1% relative error
            let tol = (expected.abs() * 0.001).max(0.001);
            assert!(
                diff <= tol,
                "mismatch at index {}: expected {}, got {}",
                i,
                expected,
                got
            );
        }
    }

    #[cfg(feature = "f16")]
    #[test]
    fn test_convert_f32_to_f16() {
        let input: Vec<f32> = (-128..128).map(|i| i as f32 * 0.125).collect();
        let output = convert_f32_to_f16_simd(&input);

        assert_eq!(output.len(), input.len());
        for (i, (&src, &dst)) in input.iter().zip(output.iter()).enumerate() {
            let roundtrip = f32::from(dst);
            let diff = (src - roundtrip).abs();
            let tol = (src.abs() * 0.001).max(0.001);
            assert!(
                diff <= tol,
                "mismatch at index {}: src {}, roundtrip {}",
                i,
                src,
                roundtrip
            );
        }
    }

    #[cfg(feature = "f16")]
    #[test]
    fn test_f16_roundtrip_simd_vs_scalar() {
        use crate::f16;
        let input: Vec<f32> = (-128..128).map(|i| i as f32 * 0.125).collect();
        let input_f16: Vec<f16> = input.iter().map(|&v| f16::from_f32(v)).collect();

        // SIMD f16→f32
        let simd_f32 = convert_f16_to_f32_simd(&input_f16);
        // Scalar f16→f32
        let scalar_f32: Vec<f32> = input_f16.iter().map(|&v| f32::from(v)).collect();

        assert_eq!(simd_f32.len(), scalar_f32.len());
        for (i, (&s, &c)) in simd_f32.iter().zip(scalar_f32.iter()).enumerate() {
            let diff = (s - c).abs();
            assert!(
                diff < 1e-6,
                "mismatch at index {}: simd={}, scalar={}",
                i,
                s,
                c
            );
        }

        // SIMD f32→f16
        let simd_f16 = convert_f32_to_f16_simd(&input);
        // Scalar f32→f16
        let scalar_f16: Vec<f16> = input.iter().map(|&v| f16::from_f32(v)).collect();

        assert_eq!(simd_f16.len(), scalar_f16.len());
        for (i, (&s, &c)) in simd_f16.iter().zip(scalar_f16.iter()).enumerate() {
            assert_eq!(s.to_bits(), c.to_bits(), "mismatch at index {}", i);
        }
    }

    // ── Byte-swap tests ────────────────────────────────────────────

    fn scalar_swap_2byte(src: &[u8]) -> Vec<u8> {
        let mut dst = vec![0u8; src.len()];
        for (i, chunk) in src.chunks_exact(2).enumerate() {
            dst[i * 2] = chunk[1];
            dst[i * 2 + 1] = chunk[0];
        }
        dst
    }

    fn scalar_swap_4byte(src: &[u8]) -> Vec<u8> {
        let mut dst = vec![0u8; src.len()];
        for (i, chunk) in src.chunks_exact(4).enumerate() {
            dst[i * 4] = chunk[3];
            dst[i * 4 + 1] = chunk[2];
            dst[i * 4 + 2] = chunk[1];
            dst[i * 4 + 3] = chunk[0];
        }
        dst
    }

    fn scalar_swap_8byte(src: &[u8]) -> Vec<u8> {
        let mut dst = vec![0u8; src.len()];
        for (i, chunk) in src.chunks_exact(8).enumerate() {
            dst[i * 8] = chunk[7];
            dst[i * 8 + 1] = chunk[6];
            dst[i * 8 + 2] = chunk[5];
            dst[i * 8 + 3] = chunk[4];
            dst[i * 8 + 4] = chunk[3];
            dst[i * 8 + 5] = chunk[2];
            dst[i * 8 + 6] = chunk[1];
            dst[i * 8 + 7] = chunk[0];
        }
        dst
    }

    #[test]
    fn test_swap_2byte_simd() {
        let input: Vec<u8> = (0..128).collect();
        let mut simd_out = vec![0u8; input.len()];
        swap_2byte_simd(&input, &mut simd_out);
        let expected = scalar_swap_2byte(&input);
        assert_eq!(simd_out, expected);
    }

    #[test]
    fn test_swap_4byte_simd() {
        let input: Vec<u8> = (0..128).collect();
        let mut simd_out = vec![0u8; input.len()];
        swap_4byte_simd(&input, &mut simd_out);
        let expected = scalar_swap_4byte(&input);
        assert_eq!(simd_out, expected);
    }

    #[test]
    fn test_swap_8byte_simd() {
        let input: Vec<u8> = (0..128).collect();
        let mut simd_out = vec![0u8; input.len()];
        swap_8byte_simd(&input, &mut simd_out);
        let expected = scalar_swap_8byte(&input);
        assert_eq!(simd_out, expected);
    }

    #[test]
    fn test_swap_2byte_identity() {
        let input: Vec<u8> = (0..128).collect();
        let mut tmp = vec![0u8; input.len()];
        let mut back = vec![0u8; input.len()];
        swap_2byte_simd(&input, &mut tmp);
        swap_2byte_simd(&tmp, &mut back);
        assert_eq!(back, input);
    }

    #[test]
    fn test_swap_4byte_identity() {
        let input: Vec<u8> = (0..128).collect();
        let mut tmp = vec![0u8; input.len()];
        let mut back = vec![0u8; input.len()];
        swap_4byte_simd(&input, &mut tmp);
        swap_4byte_simd(&tmp, &mut back);
        assert_eq!(back, input);
    }

    #[test]
    fn test_swap_8byte_identity() {
        let input: Vec<u8> = (0..128).collect();
        let mut tmp = vec![0u8; input.len()];
        let mut back = vec![0u8; input.len()];
        swap_8byte_simd(&input, &mut tmp);
        swap_8byte_simd(&tmp, &mut back);
        assert_eq!(back, input);
    }

    #[test]
    fn test_swap_2byte_short() {
        let input: Vec<u8> = (0..34).collect();
        let mut simd_out = vec![0u8; input.len()];
        swap_2byte_simd(&input, &mut simd_out);
        let expected = scalar_swap_2byte(&input);
        assert_eq!(simd_out, expected);
    }

    #[test]
    fn test_swap_4byte_short() {
        let input: Vec<u8> = (0..36).collect();
        let mut simd_out = vec![0u8; input.len()];
        swap_4byte_simd(&input, &mut simd_out);
        let expected = scalar_swap_4byte(&input);
        assert_eq!(simd_out, expected);
    }

    #[test]
    fn test_swap_8byte_short() {
        let input: Vec<u8> = (0..40).collect();
        let mut simd_out = vec![0u8; input.len()];
        swap_8byte_simd(&input, &mut simd_out);
        let expected = scalar_swap_8byte(&input);
        assert_eq!(simd_out, expected);
    }

    // ── f32 statistics tests ───────────────────────────────────────────

    #[test]
    fn test_stats_f32_simd_basic() {
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (min, max, mean, rms) = stats_f32_simd(&data);
        assert_eq!(min, 1.0);
        assert_eq!(max, 5.0);
        assert_eq!(mean, 3.0);
        // pop stddev of [1,2,3,4,5] = sqrt(2.0) ≈ 1.4142
        assert!((rms - std::f32::consts::SQRT_2).abs() < 1e-5);
    }

    #[test]
    fn test_stats_f32_simd_large() {
        let data: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let (min, max, mean, rms) = stats_f32_simd(&data);
        assert_eq!(min, 0.0);
        assert_eq!(max, 999.0);
        assert!((mean - 499.5).abs() < 1e-6);
        // pop stddev of uniform 0..999 ≈ 288.675
        assert!((rms - 288.67493).abs() < 1.0);
    }

    #[test]
    fn test_stats_f32_simd_single_element() {
        let data = vec![42.0f32];
        let (min, max, mean, rms) = stats_f32_simd(&data);
        assert_eq!(min, 42.0);
        assert_eq!(max, 42.0);
        assert_eq!(mean, 42.0);
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn test_stats_f32_simd_empty() {
        let data: Vec<f32> = vec![];
        let (min, max, mean, rms) = stats_f32_simd(&data);
        assert_eq!(min, 0.0);
        assert_eq!(max, -1.0);
        assert_eq!(mean, -2.0);
        assert_eq!(rms, -1.0);
    }

    #[test]
    fn test_stats_f32_simd_vs_scalar() {
        let data: Vec<f32> = (0..500).map(|i| (i as f32) * 0.75 + 1.0).collect();
        let simd_result = stats_f32_simd(&data);
        let scalar_result = stats_f32_scalar(&data);
        assert!((simd_result.0 - scalar_result.0).abs() < 1e-6);
        assert!((simd_result.1 - scalar_result.1).abs() < 1e-6);
        assert!((simd_result.2 - scalar_result.2).abs() < 1e-6);
        assert!((simd_result.3 - scalar_result.3).abs() < 1e-6);
    }

    #[test]
    fn test_stats_f32_simd_umaligned() {
        // Test with non-SIMD-multiple sizes
        for size in [1, 2, 3, 5, 7, 9, 15, 17, 33] {
            let data: Vec<f32> = (0..size).map(|i| i as f32).collect();
            let simd_r = stats_f32_simd(&data);
            let scalar_r = stats_f32_scalar(&data);
            assert!((simd_r.0 - scalar_r.0).abs() < 1e-6, "size={}", size);
            assert!((simd_r.1 - scalar_r.1).abs() < 1e-6, "size={}", size);
            assert!((simd_r.2 - scalar_r.2).abs() < 1e-6, "size={}", size);
            assert!((simd_r.3 - scalar_r.3).abs() < 1e-6, "size={}", size);
        }
    }
}
