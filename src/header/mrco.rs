// ============================================================================
// MRCO — legacy MRC format
// ============================================================================

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Size of a single MRCO record, in bytes.
pub const MRCO_RECORD_SIZE: usize = 80;

/// A legacy MRCO extended header record.
///
/// The MRCO format uses the same 80-byte record size as CCP4 but stores
/// different metadata.  The exact byte layout is not fully standardised
/// across implementations; this struct stores the raw bytes for
/// interpretation by the caller.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct MrcoRecord {
    /// Raw 80-byte MRCO record.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_byte_array"))]
    pub raw: [u8; MRCO_RECORD_SIZE],
}

impl MrcoRecord {
    /// Parse a single MRCO record from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < MRCO_RECORD_SIZE {
            return None;
        }
        let mut raw = [0u8; MRCO_RECORD_SIZE];
        raw.copy_from_slice(&bytes[..MRCO_RECORD_SIZE]);
        Some(Self { raw })
    }
}

/// Parse extended header bytes as MRCO records.
pub fn parse_mrco_records(bytes: &[u8]) -> Option<Vec<MrcoRecord>> {
    if bytes.is_empty() || bytes.len() % MRCO_RECORD_SIZE != 0 {
        return None;
    }
    let count = bytes.len() / MRCO_RECORD_SIZE;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * MRCO_RECORD_SIZE;
        records.push(MrcoRecord::from_bytes(
            &bytes[start..start + MRCO_RECORD_SIZE],
        )?);
    }
    Some(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mrco_roundtrip() {
        let buf = vec![42u8; MRCO_RECORD_SIZE];
        let records = super::parse_mrco_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].raw[0], 42);
    }

    #[test]
    fn mrco_empty() {
        assert!(super::parse_mrco_records(&[]).is_none());
    }
}
