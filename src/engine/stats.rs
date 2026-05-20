//! Statistics computation for MRC data validation.

use crate::engine::codec::decode_slice;
use crate::engine::endian::FileEndian;
use crate::mode::{Float32Complex, Int16Complex, Mode};

/// Compute (dmin, dmax, dmean, rms) from raw data bytes.
///
/// Returns sentinel values `(0.0, -1.0, -2.0, -1.0)` for empty data.
pub fn compute_stats(bytes: &[u8], mode: Mode, endian: FileEndian) -> (f32, f32, f32, f32) {
    match mode {
        Mode::Float32 => {
            let data = decode_slice::<f32>(bytes, endian);
            stats_real(&data)
        }
        Mode::Int16 => {
            let data = decode_slice::<i16>(bytes, endian);
            stats_real(&data)
        }
        Mode::Uint16 => {
            let data = decode_slice::<u16>(bytes, endian);
            stats_real(&data)
        }
        Mode::Int8 => {
            let data = decode_slice::<i8>(bytes, endian);
            stats_real(&data)
        }
        Mode::Float32Complex => {
            let data = decode_slice::<Float32Complex>(bytes, endian);
            let rms = rms_complex_f32(&data);
            (0.0, -1.0, -2.0, rms)
        }
        Mode::Int16Complex => {
            let data = decode_slice::<Int16Complex>(bytes, endian);
            let rms = rms_complex_i16(&data);
            (0.0, -1.0, -2.0, rms)
        }
        #[cfg(feature = "f16")]
        Mode::Float16 => {
            let data = decode_slice::<f16>(bytes, endian);
            let data_f32: Vec<f32> = data.iter().map(|&v| v as f32).collect();
            stats_real(&data_f32)
        }
        Mode::Packed4Bit => {
            // Packed4Bit: each byte holds 2 values (low nibble, high nibble)
            let num_values = bytes.len() * 2;
            let unpacked = crate::engine::convert::unpack_u4_bytes_to_u16(bytes, num_values);
            stats_real(&unpacked)
        }
    }
}

fn stats_real<T>(data: &[T]) -> (f32, f32, f32, f32)
where
    T: Copy + Into<f64>,
{
    if data.is_empty() {
        return (0.0, -1.0, -2.0, -1.0);
    }
    let iter = || data.iter().copied().map(Into::<f64>::into);
    let min = iter().fold(f64::INFINITY, f64::min) as f32;
    let max = iter().fold(f64::NEG_INFINITY, f64::max) as f32;
    let sum: f64 = iter().sum();
    let mean = (sum / data.len() as f64) as f32;
    let variance: f64 = iter()
        .map(|v| {
            let d = v - mean as f64;
            d * d
        })
        .sum::<f64>()
        / data.len() as f64;
    let rms = variance.sqrt() as f32;
    (min, max, mean, rms)
}

fn rms_complex_f32(data: &[Float32Complex]) -> f32 {
    if data.is_empty() {
        return -1.0;
    }
    let mean_real = data.iter().map(|c| c.real as f64).sum::<f64>() / data.len() as f64;
    let mean_imag = data.iter().map(|c| c.imag as f64).sum::<f64>() / data.len() as f64;
    let variance: f64 = data
        .iter()
        .map(|c| {
            let dr = c.real as f64 - mean_real;
            let di = c.imag as f64 - mean_imag;
            dr * dr + di * di
        })
        .sum::<f64>()
        / data.len() as f64;
    variance.sqrt() as f32
}

fn rms_complex_i16(data: &[Int16Complex]) -> f32 {
    if data.is_empty() {
        return -1.0;
    }
    let mean_real = data.iter().map(|c| c.real as f64).sum::<f64>() / data.len() as f64;
    let mean_imag = data.iter().map(|c| c.imag as f64).sum::<f64>() / data.len() as f64;
    let variance: f64 = data
        .iter()
        .map(|c| {
            let dr = c.real as f64 - mean_real;
            let di = c.imag as f64 - mean_imag;
            dr * dr + di * di
        })
        .sum::<f64>()
        / data.len() as f64;
    variance.sqrt() as f32
}

/// Check whether two f32 values are "close" within a relative tolerance.
///
/// Uses the same logic as Python's `np.isclose(rtol=0.01, atol=0.0)`:
/// `|a - b| <= rtol * max(|a|, |b|)`.
pub fn is_close(a: f32, b: f32, rtol: f32) -> bool {
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
pub fn validate_header_stats(header: &crate::Header, data_bytes: &[u8]) -> Result<(), crate::Error> {
    let endian = header.detect_endian();
    let mode = crate::Mode::from_i32(header.mode).unwrap_or(crate::Mode::Float32);
    let (actual_dmin, actual_dmax, actual_dmean, actual_rms) =
        compute_stats(data_bytes, mode, endian);

    let rtol = 0.01f32;

    // For complex modes, dmin/dmax/dmean are not meaningful (sentinel values)
    let complex = matches!(mode, crate::Mode::Float32Complex | crate::Mode::Int16Complex);

    // Sentinel values indicating stats have not been calculated.
    let min_unset = header.dmin == 0.0 && header.dmax == -1.0 && header.dmean == -2.0;
    let rms_unset = header.rms == -1.0;

    let min_ok = complex || min_unset || is_close(header.dmin, actual_dmin, rtol);
    let max_ok = complex || min_unset || is_close(header.dmax, actual_dmax, rtol);
    let mean_ok = complex || min_unset || is_close(header.dmean, actual_dmean, rtol);
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
        let (min, max, mean, _rms) = compute_stats(&bytes, Mode::Float32, FileEndian::LittleEndian);
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
