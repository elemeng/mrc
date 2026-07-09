// ============================================================================
// SERI — SerialEM format
// ============================================================================

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Size of a single SERI (SerialEM) record, in bytes.
pub const SERI_RECORD_SIZE: usize = 256;

/// SerialEM extended header record.
///
/// SerialEM stores tilt-series metadata in 256-byte records.  The format
/// is documented in the IMOD documentation at
/// <http://bio3d.colorado.edu/imod/doc/mrc_format.txt>.
///
/// The tilt angle at bytes 0–3 (little-endian `f32`) is the most commonly
/// accessed field and is exposed directly.  All other fields are accessible
/// via the [`raw`](SeriRecord::raw) byte array.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SeriRecord {
    /// Tilt angle in degrees, read from bytes 0–3 as little-endian `f32`.
    /// This is the single most commonly accessed field in SerialEM records.
    pub alpha_tilt: f32,
    /// Raw 256-byte record — access unparsed fields via this slice.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_byte_array"))]
    pub raw: [u8; SERI_RECORD_SIZE],
}

impl SeriRecord {
    /// Parse a single SerialEM record from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < SERI_RECORD_SIZE {
            return None;
        }
        let mut raw = [0u8; SERI_RECORD_SIZE];
        raw.copy_from_slice(&bytes[..SERI_RECORD_SIZE]);
        let alpha_tilt = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Some(Self { raw, alpha_tilt })
    }
}

crate::impl_record_parser!(SeriRecord, SERI_RECORD_SIZE, parse_seri_records);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seri_roundtrip() {
        let mut buf = vec![0u8; SERI_RECORD_SIZE];
        buf[0..4].copy_from_slice(&(-35.5f32).to_le_bytes());
        buf[4] = 99;
        let records = super::parse_seri_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        assert!((records[0].alpha_tilt - (-35.5)).abs() < 1e-6);
        assert_eq!(records[0].raw[4], 99);
    }

    #[test]
    fn seri_empty() {
        assert!(super::parse_seri_records(&[]).is_none());
    }
}
