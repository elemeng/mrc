#![cfg(target_arch = "aarch64")]

// =============================================================================
// AArch64 NEON implementations
// =============================================================================

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_i8_to_f32_neon(src: &[i8]) -> Vec<f32> {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_i16_to_f32_neon(src: &[i16]) -> Vec<f32> {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_u16_to_f32_neon(src: &[u16]) -> Vec<f32> {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
pub(super) unsafe fn convert_u8_to_f32_neon(src: &[u8]) -> Vec<f32> {
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

#[cfg(feature = "f16")]
#[target_feature(enable = "fp16")]
/// SAFETY: Caller must ensure fp16 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
///
/// Uses `vcvt_f32_f16` to convert 4 half-precision floats per call,
/// processing 8 elements per loop iteration.
pub(super) unsafe fn convert_f16_to_f32_neon(src: &[crate::f16]) -> Vec<f32> {
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

#[cfg(feature = "f16")]
#[target_feature(enable = "fp16")]
/// SAFETY: Caller must ensure fp16 is available at runtime. All `src.len()`
/// elements are initialized before `set_len` — SIMD loop + tail write loop.
///
/// Uses `vcvt_f16_f32` to convert 4 single-precision floats per call,
/// processing 8 elements per loop iteration.
pub(super) unsafe fn convert_f32_to_f16_neon(src: &[f32]) -> Vec<crate::f16> {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime.
/// Swaps every 2-byte pair using vrev16q_u8.
pub(super) unsafe fn swap_2byte_neon(src: &[u8], dst: &mut [u8]) {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime.
/// Swaps every 4-byte group using vrev32q_u8.
pub(super) unsafe fn swap_4byte_neon(src: &[u8], dst: &mut [u8]) {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available at runtime.
/// Swaps every 8-byte group using vrev64q_u8.
pub(super) unsafe fn swap_8byte_neon(src: &[u8], dst: &mut [u8]) {
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available.
/// Two-pass SIMD f32 statistics: pass 1 = min/max/sum, pass 2 = variance.
pub(super) unsafe fn stats_f32_neon(data: &[f32]) -> (f32, f32, f32, f32) {
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

// ── NEON implementations (write-side) ─────────────────────────────────

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available. All elements initialized before set_len.
pub(super) unsafe fn convert_f32_to_i16_neon(src: &[f32]) -> Vec<i16> {
    use core::arch::aarch64::*;

    let mut dst: Vec<i16> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;
    let vmin = vdupq_n_f32(i16::MIN as f32);
    let vmax = vdupq_n_f32(i16::MAX as f32);

    while i + 4 <= src.len() {
        let v = vld1q_f32(src.as_ptr().add(i));
        // Replace NaN with 0 using the fact that NaN != NaN
        let isnan = vreinterpretq_u32_f32(vcgtzq_f32(vsubq_f32(v, v)));
        let v_ok = vreinterpretq_f32_u32(vbicq_u32(vreinterpretq_u32_f32(v), isnan));
        // Clamp and convert
        let clamped = vminq_f32(vmaxq_f32(v_ok, vmin), vmax);
        let i32x4 = vcvtq_s32_f32(clamped);
        let i16x4 = vqmovn_s32(i32x4); // signed saturate i32→i16
        vst1_s16(dst_ptr.add(i), i16x4);
        i += 4;
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available. All elements initialized before set_len.
pub(super) unsafe fn convert_f32_to_u16_neon(src: &[f32]) -> Vec<u16> {
    use core::arch::aarch64::*;

    let mut dst: Vec<u16> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;
    let vmax = vdupq_n_f32(u16::MAX as f32);
    let zero = vdupq_n_f32(0.0);

    while i + 4 <= src.len() {
        let v = vld1q_f32(src.as_ptr().add(i));
        let isnan = vreinterpretq_u32_f32(vcgtzq_f32(vsubq_f32(v, v)));
        let v_ok = vreinterpretq_f32_u32(vbicq_u32(vreinterpretq_u32_f32(v), isnan));
        let clamped = vmaxq_f32(vminq_f32(v_ok, vmax), zero);
        let i32x4 = vcvtq_s32_f32(clamped);
        let u16x4 = vqmovun_s32(i32x4); // unsigned saturate i32→u16
        vst1_u16(dst_ptr.add(i), u16x4);
        i += 4;
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

#[target_feature(enable = "neon")]
/// SAFETY: Caller must ensure NEON is available. All elements initialized before set_len.
pub(super) unsafe fn convert_f32_to_i8_neon(src: &[f32]) -> Vec<i8> {
    use core::arch::aarch64::*;

    let mut dst: Vec<i8> = Vec::with_capacity(src.len());
    let dst_ptr = dst.as_mut_ptr();
    let mut i = 0;
    let vmin = vdupq_n_f32(i8::MIN as f32);
    let vmax = vdupq_n_f32(i8::MAX as f32);

    // Process 8 elements at a time (narrow i32→i16→i8)
    while i + 8 <= src.len() {
        let v0 = vld1q_f32(src.as_ptr().add(i));
        let isnan0 = vreinterpretq_u32_f32(vcgtzq_f32(vsubq_f32(v0, v0)));
        let v0_ok = vreinterpretq_f32_u32(vbicq_u32(vreinterpretq_u32_f32(v0), isnan0));
        let c0 = vminq_f32(vmaxq_f32(v0_ok, vmin), vmax);
        let i32_0 = vcvtq_s32_f32(c0);

        let v1 = vld1q_f32(src.as_ptr().add(i + 4));
        let isnan1 = vreinterpretq_u32_f32(vcgtzq_f32(vsubq_f32(v1, v1)));
        let v1_ok = vreinterpretq_f32_u32(vbicq_u32(vreinterpretq_u32_f32(v1), isnan1));
        let c1 = vminq_f32(vmaxq_f32(v1_ok, vmin), vmax);
        let i32_1 = vcvtq_s32_f32(c1);

        // Narrow: 8 i32 → 8 i16 → 8 i8
        let i16x8 = vcombine_s16(vqmovn_s32(i32_0), vqmovn_s32(i32_1));
        let i8x8 = vqmovn_s16(i16x8);
        vst1_s8(dst_ptr.add(i), i8x8);
        i += 8;
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
