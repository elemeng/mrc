#![cfg(target_arch = "x86_64")]

// =============================================================================
// x86_64 AVX2 implementations
// =============================================================================

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. This function:
/// - Allocates `Vec::with_capacity(src.len())` — enough for all elements
/// - Fills elements via SIMD stores in the loop and the scalar tail loop
/// - Calls `set_len` only after all elements are initialized
/// - Uses unaligned load/store intrinsics which do not require aligned pointers
pub(super) unsafe fn convert_i8_to_f32_avx2(src: &[i8]) -> Vec<f32> {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_i16_to_f32_avx2(src: &[i16]) -> Vec<f32> {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_u16_to_f32_avx2(src: &[u16]) -> Vec<f32> {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_u8_to_f32_avx2(src: &[u8]) -> Vec<f32> {
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

#[cfg(feature = "f16")]
#[target_feature(enable = "f16c")]
/// SAFETY: Caller must ensure F16C is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_f16_to_f32_avx2(src: &[crate::f16]) -> Vec<f32> {
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

#[cfg(feature = "f16")]
#[target_feature(enable = "f16c")]
/// SAFETY: Caller must ensure F16C is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_f32_to_f16_avx2(src: &[f32]) -> Vec<crate::f16> {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime.
/// Swaps every 2-byte pair using PSHUFB.
pub(super) unsafe fn swap_2byte_avx2(src: &[u8], dst: &mut [u8]) {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime.
/// Swaps every 4-byte group using PSHUFB.
pub(super) unsafe fn swap_4byte_avx2(src: &[u8], dst: &mut [u8]) {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available at runtime.
/// Swaps every 8-byte group using PSHUFB.
pub(super) unsafe fn swap_8byte_avx2(src: &[u8], dst: &mut [u8]) {
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

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available.
/// Two-pass SIMD f32 statistics: pass 1 = min/max/sum, pass 2 = variance.
pub(super) unsafe fn stats_f32_avx2(data: &[f32]) -> (f32, f32, f32, f32) {
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

// ── AVX2 implementations (write-side) ──────────────────────────────────

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available. All elements initialized before set_len.
pub(super) unsafe fn convert_f32_to_i16_avx2(src: &[f32]) -> Vec<i16> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<i16> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;
        let vmin = _mm256_set1_ps(i16::MIN as f32);
        let vmax = _mm256_set1_ps(i16::MAX as f32);
        let zero = _mm256_setzero_ps();

        while i + 8 <= src.len() {
            let v = _mm256_loadu_ps(src.as_ptr().add(i));
            // Zero out NaN values (cmp + blend)
            let nan = _mm256_cmp_ps(v, v, _CMP_UNORD_Q);
            let v_ok = _mm256_blendv_ps(v, zero, nan);
            // Clamp and convert to i32
            let clamped = _mm256_min_ps(_mm256_max_ps(v_ok, vmin), vmax);
            let i32x8 = _mm256_cvtps_epi32(clamped);
            // Narrow i32→i16 with signed saturation via SSE2 pack
            let lo = _mm256_castsi256_si128(i32x8);
            let hi = _mm256_extracti128_si256(i32x8, 1);
            let i16x8 = _mm_packs_epi32(lo, hi);
            _mm_storeu_si128(dst_ptr.add(i) as *mut __m128i, i16x8);
            i += 8;
        }

        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = if v.is_nan() {
                0
            } else {
                v.clamp(i16::MIN as f32, i16::MAX as f32) as i16
            };
        }

        dst.set_len(src.len());
        dst
    }
}

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available. All elements initialized before set_len.
pub(super) unsafe fn convert_f32_to_u16_avx2(src: &[f32]) -> Vec<u16> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<u16> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;
        let vmax = _mm256_set1_ps(u16::MAX as f32);
        let zero = _mm256_setzero_ps();

        while i + 8 <= src.len() {
            let v = _mm256_loadu_ps(src.as_ptr().add(i));
            // Zero out NaN, clamp to [0, u16::MAX]
            let nan = _mm256_cmp_ps(v, v, _CMP_UNORD_Q);
            let v_ok = _mm256_blendv_ps(v, zero, nan);
            let clamped = _mm256_min_ps(_mm256_max_ps(v_ok, zero), vmax);
            let i32x8 = _mm256_cvtps_epi32(clamped);
            let lo = _mm256_castsi256_si128(i32x8);
            let hi = _mm256_extracti128_si256(i32x8, 1);
            // Unsigned saturation i32→u16
            let u16x8 = _mm_packus_epi32(lo, hi);
            _mm_storeu_si128(dst_ptr.add(i) as *mut __m128i, u16x8);
            i += 8;
        }

        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = if v.is_nan() {
                0
            } else {
                v.clamp(0.0, u16::MAX as f32) as u16
            };
        }

        dst.set_len(src.len());
        dst
    }
}

#[target_feature(enable = "avx2")]
/// SAFETY: Caller must ensure AVX2 is available. All elements initialized before set_len.
pub(super) unsafe fn convert_f32_to_i8_avx2(src: &[f32]) -> Vec<i8> {
    unsafe {
        use core::arch::x86_64::*;

        let mut dst: Vec<i8> = Vec::with_capacity(src.len());
        let dst_ptr = dst.as_mut_ptr();
        let mut i = 0;
        let vmin = _mm256_set1_ps(i8::MIN as f32);
        let vmax = _mm256_set1_ps(i8::MAX as f32);
        let zero = _mm256_setzero_ps();

        // Process 16 elements at a time (two rounds of narrowing)
        while i + 16 <= src.len() {
            // First 8
            let v0 = _mm256_loadu_ps(src.as_ptr().add(i));
            let nan0 = _mm256_cmp_ps(v0, v0, _CMP_UNORD_Q);
            let v0_ok = _mm256_blendv_ps(v0, zero, nan0);
            let c0 = _mm256_min_ps(_mm256_max_ps(v0_ok, vmin), vmax);
            let i32_0 = _mm256_cvtps_epi32(c0);

            // Second 8
            let v1 = _mm256_loadu_ps(src.as_ptr().add(i + 8));
            let nan1 = _mm256_cmp_ps(v1, v1, _CMP_UNORD_Q);
            let v1_ok = _mm256_blendv_ps(v1, zero, nan1);
            let c1 = _mm256_min_ps(_mm256_max_ps(v1_ok, vmin), vmax);
            let i32_1 = _mm256_cvtps_epi32(c1);

            // Narrow: 16 i32 → 16 i16 (signed sat) → 16 i8 (signed sat)
            let i16_lo = _mm_packs_epi32(
                _mm256_castsi256_si128(i32_0),
                _mm256_extracti128_si256(i32_0, 1),
            );
            let i16_hi = _mm_packs_epi32(
                _mm256_castsi256_si128(i32_1),
                _mm256_extracti128_si256(i32_1, 1),
            );
            let i8x16 = _mm_packs_epi16(i16_lo, i16_hi);
            _mm_storeu_si128(dst_ptr.add(i) as *mut __m128i, i8x16);
            i += 16;
        }

        for (j, &v) in src.iter().enumerate().skip(i) {
            *dst_ptr.add(j) = if v.is_nan() {
                0
            } else {
                v.clamp(i8::MIN as f32, i8::MAX as f32) as i8
            };
        }

        dst.set_len(src.len());
        dst
    }
}
