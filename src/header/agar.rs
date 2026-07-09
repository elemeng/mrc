// ============================================================================
// AGAR — Agard format
// ============================================================================

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Size of a single AGAR record, in bytes.
pub const AGAR_RECORD_SIZE: usize = 1024;

/// Agard extended header record.
///
/// Agard extended headers use 1024-byte records.  This format is used by
/// older Agard-style microscopes.  Access the raw bytes for field parsing.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct AgarRecord {
    /// Raw 1024-byte record.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_byte_array"))]
    pub raw: [u8; AGAR_RECORD_SIZE],
}

impl AgarRecord {
    /// Parse a single Agard record from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < AGAR_RECORD_SIZE {
            return None;
        }
        let mut raw = [0u8; AGAR_RECORD_SIZE];
        raw.copy_from_slice(&bytes[..AGAR_RECORD_SIZE]);
        Some(Self { raw })
    }
}

crate::impl_record_parser!(AgarRecord, AGAR_RECORD_SIZE, parse_agar_records);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agar_roundtrip() {
        let buf = vec![77u8; AGAR_RECORD_SIZE];
        let records = super::parse_agar_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].raw[0], 77);
    }

    #[test]
    fn agar_empty() {
        assert!(super::parse_agar_records(&[]).is_none());
    }
}
