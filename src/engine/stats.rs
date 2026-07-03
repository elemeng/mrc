//! Statistics computation for MRC data validation.
//!
//! Computes `(dmin, dmax, dmean, rms)` from raw MRC data bytes, respecting
//! the file's [`Mode`] and endianness. Used internally by
//! [`Reader::validate_header_stats`](crate::Reader::validate_header_stats) and
//! [`validate_full`](crate::validate::validate_full) to cross-check header
//! density statistics against actual voxel data.
//!
//! Handles all modes: Int8, Int16, Uint16, Float32, Float16 (with `f16`
//! feature), Float32Complex, Int16Complex, and Packed4Bit. Complex modes
//! compute RMS only (dmin/dmax/dmean sentinels are set).

use crate::Error;
use crate::engine::codec::decode_slice;
use crate::engine::endian::FileEndian;
use crate::mode::{Float32Complex, Int16Complex, Mode};

/// Compute (dmin, dmax, dmean, rms) from raw data bytes.
///
/// `nx` and `ny` are the volume dimensions (needed for row-by-row decoding
/// of [`Mode::Packed4Bit`]; for other modes they are unused).
///
/// Returns sentinel values `(0.0, -1.0, -2.0, -1.0)` for empty data.
///
/// # Errors
/// Returns `Error::TypeMismatch` if the byte slice cannot be decoded for the given mode.
pub(crate) fn compute_stats(
    bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
    nx: usize,
    ny: usize,
) -> Result<(f32, f32, f32, f32), Error> {
    Ok(match mode {
        Mode::Float32 => {
            let data = decode_slice::<f32>(bytes, endian)?;
            stats_real(&data)
        }
        Mode::Int16 => {
            let data = decode_slice::<i16>(bytes, endian)?;
            stats_real(&data)
        }
        Mode::Uint16 => {
            let data = decode_slice::<u16>(bytes, endian)?;
            stats_real(&data)
        }
        Mode::Int8 => {
            let data = decode_slice::<i8>(bytes, endian)?;
            stats_real(&data)
        }
        Mode::Float32Complex => {
            let data = decode_slice::<Float32Complex>(bytes, endian)?;
            let rms = rms_complex_f32(&data);
            (0.0, -1.0, -2.0, rms)
        }
        Mode::Int16Complex => {
            let data = decode_slice::<Int16Complex>(bytes, endian)?;
            let rms = rms_complex_i16(&data);
            (0.0, -1.0, -2.0, rms)
        }
        #[cfg(feature = "f16")]
        Mode::Float16 => {
            let data = decode_slice::<crate::f16>(bytes, endian)?;
            let data_f32 = crate::engine::convert::convert_f16_slice_to_f32(&data);
            stats_real(&data_f32)
        }
        #[cfg(not(feature = "f16"))]
        Mode::Float16 => return Err(Error::UnsupportedMode),
        Mode::Packed4Bit => {
            let unpacked = crate::engine::convert::unpack_u4_bytes_to_u8(bytes, nx, ny);
            stats_real(&unpacked)
        }
    })
}

fn stats_real<T>(data: &[T]) -> (f32, f32, f32, f32)
where
    T: Copy + Into<f64> + 'static,
{
    if data.is_empty() {
        return (0.0, -1.0, -2.0, -1.0);
    }

    // Specialized SIMD path for f32 (most common case).
    #[cfg(feature = "simd")]
    {
        // SAFETY: we check the type identity via pointer comparison after
        // monomorphisation — the compiler optimises the branch away.
        if core::any::TypeId::of::<T>() == core::any::TypeId::of::<f32>() {
            let f32_data: &[f32] =
                unsafe { core::slice::from_raw_parts(data.as_ptr() as *const f32, data.len()) };
            return stats_f32_simd_inner(f32_data);
        }
    }

    // Generic single-pass scalar using Welford's online algorithm.
    let len = data.len();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut n = 0u64;
    let mut mean = 0.0f64;
    let mut m2 = 0.0f64;

    for &v in data {
        let x = v.into();
        n += 1;
        if x < min {
            min = x;
        }
        if x > max {
            max = x;
        }
        let delta = x - mean;
        mean += delta / n as f64;
        m2 += delta * (x - mean);
    }

    let rms = (m2 / len as f64).sqrt();
    (min as f32, max as f32, mean as f32, rms as f32)
}

/// SIMD-accelerated single-pass statistics for f32 data.
#[cfg(feature = "simd")]
fn stats_f32_simd_inner(data: &[f32]) -> (f32, f32, f32, f32) {
    use crate::engine::simd::stats_f32_simd;
    stats_f32_simd(data)
}

fn rms_complex_f32(data: &[Float32Complex]) -> f32 {
    if data.is_empty() {
        return -1.0;
    }
    let mut sum_real = 0.0f64;
    let mut sum_imag = 0.0f64;
    for c in data {
        sum_real += c.real as f64;
        sum_imag += c.imag as f64;
    }
    let mean_real = sum_real / data.len() as f64;
    let mean_imag = sum_imag / data.len() as f64;
    let mut variance_sum = 0.0f64;
    for c in data {
        let dr = c.real as f64 - mean_real;
        let di = c.imag as f64 - mean_imag;
        variance_sum += dr * dr + di * di;
    }
    ((variance_sum / data.len() as f64).sqrt()) as f32
}

fn rms_complex_i16(data: &[Int16Complex]) -> f32 {
    if data.is_empty() {
        return -1.0;
    }
    let mut sum_real = 0.0f64;
    let mut sum_imag = 0.0f64;
    for c in data {
        sum_real += c.real as f64;
        sum_imag += c.imag as f64;
    }
    let mean_real = sum_real / data.len() as f64;
    let mean_imag = sum_imag / data.len() as f64;
    let mut variance_sum = 0.0f64;
    for c in data {
        let dr = c.real as f64 - mean_real;
        let di = c.imag as f64 - mean_imag;
        variance_sum += dr * dr + di * di;
    }
    ((variance_sum / data.len() as f64).sqrt()) as f32
}

/// Check whether two f32 values are "close" within a relative tolerance.
///
/// Uses the same logic as Python's `np.isclose(rtol=0.01, atol=0.0)`:
/// `|a - b| <= rtol * max(|a|, |b|)`.
pub(crate) fn is_close(a: f32, b: f32, rtol: f32) -> bool {
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    let scale = a.abs().max(b.abs());
    diff <= rtol * scale
}

/// Validate header statistics against actual data bytes.
///
/// Uses a 1 % relative tolerance (matching Python `mrcfile`'s `np.isclose(rtol=0.01)`).
/// For complex modes, only RMS is checked.
pub(crate) fn validate_header_stats(
    header: &crate::Header,
    data_bytes: &[u8],
) -> Result<(), crate::Error> {
    let endian = header.detect_endian();
    let mode = match crate::Mode::from_i32(header.mode) {
        Some(m) => m,
        None => return Err(crate::Error::UnsupportedMode),
    };
    let (actual_dmin, actual_dmax, actual_dmean, actual_rms) = compute_stats(
        data_bytes,
        mode,
        endian,
        header.nx as usize,
        header.ny as usize * header.nz as usize,
    )?;

    let rtol = 0.01f32;

    // For complex modes, dmin/dmax/dmean are not meaningful (sentinel values)
    let complex = matches!(
        mode,
        crate::Mode::Float32Complex | crate::Mode::Int16Complex
    );

    // Per MRC-2014 convention, dmin > dmax indicates stats have not been
    // well-determined (the header builder sets dmin=0, dmax=-1 for this).
    // Using this relational check is robust — it avoids conflating legitimate
    // data values (e.g. actual dmin == 0.0) with the unset sentinel.
    let stats_unset = header.dmin > header.dmax;
    let rms_unset = header.rms < 0.0;

    let min_ok = complex || stats_unset || is_close(header.dmin, actual_dmin, rtol);
    let max_ok = complex || stats_unset || is_close(header.dmax, actual_dmax, rtol);
    let mean_ok = complex || stats_unset || is_close(header.dmean, actual_dmean, rtol);
    let rms_ok = rms_unset || is_close(header.rms, actual_rms, rtol);

    if !min_ok || !max_ok || !mean_ok || !rms_ok {
        return Err(crate::Error::StatsMismatch {
            claimed_dmin: header.dmin,
            claimed_dmax: header.dmax,
            claimed_dmean: header.dmean,
            claimed_rms: header.rms,
            actual_dmin,
            actual_dmax,
            actual_dmean,
            actual_rms,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_real_basic() {
        let data = [1.0f32, 2.0, 3.0, 4.0];
        let (min, max, mean, rms) = stats_real(&data);
        assert_eq!(min, 1.0);
        assert_eq!(max, 4.0);
        assert_eq!(mean, 2.5);
        // population stddev of [1,2,3,4] = sqrt(1.25) ≈ 1.118
        assert!((rms - 1.118034).abs() < 1e-4);
    }

    #[test]
    fn test_stats_real_empty() {
        let data: &[f32] = &[];
        let (min, max, mean, rms) = stats_real(data);
        assert_eq!(min, 0.0);
        assert_eq!(max, -1.0);
        assert_eq!(mean, -2.0);
        assert_eq!(rms, -1.0);
    }

    #[test]
    fn test_is_close_exact() {
        assert!(is_close(1.0, 1.0, 0.01));
    }

    #[test]
    fn test_is_close_within_tol() {
        assert!(is_close(100.0, 100.5, 0.01)); // 0.5% diff < 1%
        assert!(!is_close(100.0, 102.0, 0.01)); // 2% diff > 1%
    }

    #[test]
    fn test_compute_stats_float32() {
        let bytes: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|&v| v.to_le_bytes())
            .collect();
        let (min, max, mean, _rms) =
            compute_stats(&bytes, Mode::Float32, FileEndian::LittleEndian, 4, 1).unwrap();
        assert_eq!(min, 1.0);
        assert_eq!(max, 4.0);
        assert_eq!(mean, 2.5);
    }

    #[test]
    fn test_validate_header_stats_ok() {
        let mut header = crate::Header::new();
        header.mode = Mode::Float32.as_i32();
        header.dmin = 1.0;
        header.dmax = 4.0;
        header.dmean = 2.5;
        header.rms = 1.118034;

        let bytes: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|&v| v.to_le_bytes())
            .collect();
        assert!(validate_header_stats(&header, &bytes).is_ok());
    }

    #[test]
    fn test_validate_header_stats_mismatch() {
        let mut header = crate::Header::new();
        header.mode = Mode::Float32.as_i32();
        header.dmin = 0.0;
        header.dmax = 100.0;
        header.dmean = 50.0;
        header.rms = 10.0;

        let bytes: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|&v| v.to_le_bytes())
            .collect();
        assert!(validate_header_stats(&header, &bytes).is_err());
    }

    #[test]
    fn test_validate_header_stats_sentinels_ok() {
        let mut header = crate::Header::new();
        header.mode = Mode::Float32.as_i32();
        // Sentinel values should be accepted without error
        header.dmin = 0.0;
        header.dmax = -1.0;
        header.dmean = -2.0;
        header.rms = -1.0;

        let bytes: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|&v| v.to_le_bytes())
            .collect();
        assert!(validate_header_stats(&header, &bytes).is_ok());
    }
}

// ============================================================================
// RunningStats — online Welford accumulator for writer-side statistics
// ============================================================================

/// Online single-pass statistics accumulator using Welford's algorithm.
///
/// Tracks `dmin`, `dmax`, `dmean`, and `rms` incrementally without storing
/// all voxel values.  Call [`update`](RunningStats::update) for each block
/// as it is written, then call [`finalize`](RunningStats::finalize) to
/// produce the final (dmin, dmax, dmean, rms) tuple.
///
/// This avoids a full-file re-read when [`Writer::update_header_stats`]
/// is called later.
///
/// [`Writer::update_header_stats`]: crate::Writer::update_header_stats
#[derive(Debug, Clone)]
pub(crate) struct RunningStats {
    n: u64,
    min: f64,
    max: f64,
    mean: f64,
    m2: f64,
}

#[cfg_attr(not(test), allow(dead_code))]
impl RunningStats {
    /// Create a new accumulator in the initial (empty) state.
    /// `dmin` is set to +∞ and `dmax` to −∞ so the first value sets both.
    pub fn new() -> Self {
        Self {
            n: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Update statistics with a slice of `f32` voxel values.
    ///
    /// Callers should convert their typed data to `f32` before calling
    /// this method. All real-valued MRC types (`i8`, `i16`, `u16`, `f32`,
    /// `f16`) can be losslessly widened to `f32`.
    pub fn update(&mut self, data: &[f32]) {
        for &v in data {
            let x = v as f64;
            self.n += 1;

            if x < self.min {
                self.min = x;
            }
            if x > self.max {
                self.max = x;
            }

            let delta = x - self.mean;
            self.mean += delta / self.n as f64;
            let delta2 = x - self.mean;
            self.m2 += delta * delta2;
        }
    }

    /// Accumulate statistics from another `RunningStats` instance (for
    /// parallel / chunked usage).
    #[allow(dead_code)]
    pub fn merge(&mut self, other: &Self) {
        if other.n == 0 {
            return;
        }
        if self.n == 0 {
            *self = other.clone();
            return;
        }

        let n1 = self.n as f64;
        let n2 = other.n as f64;
        let n_total = self.n + other.n;

        let delta = other.mean - self.mean;
        let new_mean = (n1 * self.mean + n2 * other.mean) / (n_total as f64);

        let new_m2 = self.m2 + other.m2 + delta * delta * n1 * n2 / (n_total as f64);

        self.n = n_total;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.mean = new_mean;
        self.m2 = new_m2;
    }

    /// Produce the final `(dmin, dmax, dmean, rms)` tuple.
    ///
    /// Returns sentinel values `(0.0, -1.0, -2.0, -1.0)` when no data has
    /// been accumulated (matching the header default convention).
    pub fn finalize(&self) -> (f32, f32, f32, f32) {
        if self.n == 0 {
            return (0.0, -1.0, -2.0, -1.0);
        }
        let rms = (self.m2 / self.n as f64).sqrt();
        (
            self.min as f32,
            self.max as f32,
            self.mean as f32,
            rms as f32,
        )
    }
}

#[cfg(test)]
mod running_stats_tests {
    use super::*;

    #[test]
    fn running_stats_empty() {
        let s = RunningStats::new();
        let (dmin, dmax, dmean, rms) = s.finalize();
        assert_eq!(dmin, 0.0);
        assert_eq!(dmax, -1.0);
        assert_eq!(dmean, -2.0);
        assert_eq!(rms, -1.0);
    }

    #[test]
    fn running_stats_known_values() {
        let mut s = RunningStats::new();
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        s.update(&data);
        let (dmin, dmax, dmean, rms) = s.finalize();
        assert_eq!(dmin, 1.0);
        assert_eq!(dmax, 4.0);
        assert_eq!(dmean, 2.5);
        assert!((rms - 1.118034).abs() < 1e-4);
    }

    #[test]
    fn running_stats_i16() {
        let mut s = RunningStats::new();
        let data: Vec<i16> = vec![-100, 0, 100, 200];
        let f32_data: Vec<f32> = data.iter().map(|&v| v as f32).collect();
        s.update(&f32_data);
        let (dmin, dmax, dmean, _) = s.finalize();
        assert_eq!(dmin, -100.0);
        assert_eq!(dmax, 200.0);
        assert_eq!(dmean, 50.0);
    }

    #[test]
    fn running_stats_u16() {
        let mut s = RunningStats::new();
        let data: Vec<u16> = vec![10, 20, 30];
        let f32_data: Vec<f32> = data.iter().map(|&v| v as f32).collect();
        s.update(&f32_data);
        let (min, max, mean, _) = s.finalize();
        assert_eq!(min, 10.0);
        assert_eq!(max, 30.0);
        assert_eq!(mean, 20.0);
    }

    #[test]
    fn running_stats_merge() {
        let mut a = RunningStats::new();
        a.update(&[1.0f32, 2.0, 3.0]);

        let mut b = RunningStats::new();
        b.update(&[4.0f32, 5.0, 6.0]);

        a.merge(&b);
        let (min, max, mean, _) = a.finalize();
        assert_eq!(min, 1.0);
        assert_eq!(max, 6.0);
        assert!((mean - 3.5).abs() < 1e-6);
    }
}
