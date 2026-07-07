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

#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86;

/// Convert a slice of i8 values to f32 using SIMD acceleration.
///
/// Uses 32-lane SIMD when available (AVX2 on x86_64, NEON on AArch64).
pub(crate) fn convert_i8_to_f32_simd(src: &[i8]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { x86::convert_i8_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_i8_to_f32_neon(src) };
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
            return unsafe { x86::convert_i16_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_i16_to_f32_neon(src) };
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
            return unsafe { x86::convert_u16_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_u16_to_f32_neon(src) };
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
            return unsafe { x86::convert_u8_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_u8_to_f32_neon(src) };
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
            return unsafe { x86::convert_f16_to_f32_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("fp16") {
            return unsafe { aarch64::convert_f16_to_f32_neon(src) };
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
            return unsafe { x86::convert_f32_to_f16_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("fp16") {
            return unsafe { aarch64::convert_f32_to_f16_neon(src) };
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
            return unsafe { x86::stats_f32_avx2(data) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::stats_f32_neon(data) };
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
            return unsafe { x86::swap_2byte_avx2(src, dst) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::swap_2byte_neon(src, dst) };
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
            return unsafe { x86::swap_4byte_avx2(src, dst) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::swap_4byte_neon(src, dst) };
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
            return unsafe { x86::swap_8byte_avx2(src, dst) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::swap_8byte_neon(src, dst) };
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
// Write-side SIMD conversions — f32 → i8 / i16 / u16
// =============================================================================

/// Convert a slice of f32 values to i16 using SIMD acceleration.
///
/// Values are clamped to the representable range of i16 before conversion.
#[cfg(feature = "simd")]
pub(crate) fn convert_f32_to_i16_simd(src: &[f32]) -> Vec<i16> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { x86::convert_f32_to_i16_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_f32_to_i16_neon(src) };
        }
    }

    // Fallback to scalar
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

/// Convert a slice of f32 values to u16 using SIMD acceleration.
///
/// Values are clamped to [0, u16::MAX] before conversion.
#[cfg(feature = "simd")]
pub(crate) fn convert_f32_to_u16_simd(src: &[f32]) -> Vec<u16> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { x86::convert_f32_to_u16_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_f32_to_u16_neon(src) };
        }
    }

    // Fallback to scalar
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

/// Convert a slice of f32 values to i8 using SIMD acceleration.
///
/// Values are clamped to the representable range of i8 before conversion.
#[cfg(feature = "simd")]
pub(crate) fn convert_f32_to_i8_simd(src: &[f32]) -> Vec<i8> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { x86::convert_f32_to_i8_avx2(src) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if core::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64::convert_f32_to_i8_neon(src) };
        }
    }

    // Fallback to scalar
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
