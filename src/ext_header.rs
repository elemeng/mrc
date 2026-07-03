//! Structured parsers for extended headers beyond FEI.
//!
//! The 4-byte EXTTYP field in the MRC header identifies the format of the
//! extended header that follows the 1024-byte fixed header.  This module
//! provides typed access to the most common formats:
//!
//! | EXTTYP | Format | Record size | Typical use |
//! |--------|--------|-------------|-------------|
//! | `CCP4` | CCP4 symmetry records | 80 bytes / record | Crystallographic symmetry operators |
//! | `MRCO` | MRC legacy format | 80 bytes / record | Legacy MRC metadata |
//! | `SERI` | SerialEM | 256 bytes / record | Tilt series metadata |
//! | `AGAR` | Agard format | 1024 bytes / record | Agard-style metadata |
//!
//! # Field coverage
//!
//! - **CCP4**: Fully parsed as 80-byte text lines. The format stores space
//!   group symmetry operators as human-readable ASCII text, one operator per
//!   line separated by `*`.
//! - **MRCO**: Raw bytes only. The byte layout is not standardised across
//!   implementations. Access `raw` and interpret per your application.
//! - **SERI**: The tilt angle (`alpha_tilt`, bytes 0–3 as `f32` LE) is
//!   exposed. Other fields documented in the
//!   [IMOD mrc_format.txt](http://bio3d.colorado.edu/imod/doc/mrc_format.txt)
//!   are accessible via the `raw` byte array.
//! - **AGAR**: Raw bytes only. The byte layout is not standardised; access
//!   `raw` and interpret per your application.
//!
//! For fields not yet exposed, access the raw extended header bytes directly
//! via [`Reader::ext_header_bytes`](crate::Reader::ext_header_bytes).

// ============================================================================
// CCP4 symmetry records
// ============================================================================

/// Size of a single CCP4 symmetry record, in bytes.
pub const CCP4_RECORD_SIZE: usize = 80;

/// A single CCP4 symmetry record — an 80-character text line containing
/// space group symmetry operators.
///
/// In the CCP4 convention, the extended header stores symmetry operators
/// as human-readable text, one operator per line, separated by `*` (asterisk)
/// and grouped into 80-character lines.  This struct stores the raw text
/// of each line.
#[derive(Debug, Clone, PartialEq)]
pub struct Ccp4Record {
    /// Raw 80-byte symmetry line (may contain multiple operators separated
    /// by `*`, padded with spaces).
    pub raw: [u8; CCP4_RECORD_SIZE],
}

impl Ccp4Record {
    /// Parse a single CCP4 record from bytes.
    ///
    /// Returns `None` if `bytes` is shorter than [`CCP4_RECORD_SIZE`].
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < CCP4_RECORD_SIZE {
            return None;
        }
        let mut raw = [0u8; CCP4_RECORD_SIZE];
        raw.copy_from_slice(&bytes[..CCP4_RECORD_SIZE]);
        Some(Self { raw })
    }

    /// Return the symmetry text as a trimmed string.
    pub fn as_str(&self) -> &str {
        let end = self
            .raw
            .iter()
            .rposition(|&b| b != b' ')
            .map_or(0, |p| p + 1);
        core::str::from_utf8(&self.raw[..end]).unwrap_or("")
    }
}

/// Parse extended header bytes as CCP4 symmetry records.
///
/// Returns `None` if `bytes` is empty or if its length is not a multiple of
/// [`CCP4_RECORD_SIZE`].
pub fn parse_ccp4_records(bytes: &[u8]) -> Option<Vec<Ccp4Record>> {
    if bytes.is_empty() || bytes.len() % CCP4_RECORD_SIZE != 0 {
        return None;
    }
    let count = bytes.len() / CCP4_RECORD_SIZE;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * CCP4_RECORD_SIZE;
        records.push(Ccp4Record::from_bytes(
            &bytes[start..start + CCP4_RECORD_SIZE],
        )?);
    }
    Some(records)
}

// ============================================================================
// MRCO — legacy MRC format
// ============================================================================

/// Size of a single MRCO record, in bytes.
pub const MRCO_RECORD_SIZE: usize = 80;

/// A legacy MRCO extended header record.
///
/// The MRCO format uses the same 80-byte record size as CCP4 but stores
/// different metadata.  The exact byte layout is not fully standardised
/// across implementations; this struct stores the raw bytes for
/// interpretation by the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct MrcoRecord {
    /// Raw 80-byte MRCO record.
    pub raw: [u8; MRCO_RECORD_SIZE],
}

impl MrcoRecord {
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

// ============================================================================
// SERI — SerialEM format
// ============================================================================

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
#[derive(Debug, Clone, PartialEq)]
pub struct SeriRecord {
    /// Tilt angle in degrees, read from bytes 0–3 as little-endian `f32`.
    /// This is the single most commonly accessed field in SerialEM records.
    pub alpha_tilt: f32,
    /// Raw 256-byte record — access unparsed fields via this slice.
    pub raw: [u8; SERI_RECORD_SIZE],
}

impl SeriRecord {
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

/// Parse extended header bytes as SerialEM records.
pub fn parse_seri_records(bytes: &[u8]) -> Option<Vec<SeriRecord>> {
    if bytes.is_empty() || bytes.len() % SERI_RECORD_SIZE != 0 {
        return None;
    }
    let count = bytes.len() / SERI_RECORD_SIZE;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * SERI_RECORD_SIZE;
        records.push(SeriRecord::from_bytes(
            &bytes[start..start + SERI_RECORD_SIZE],
        )?);
    }
    Some(records)
}

// ============================================================================
// AGAR — Agard format
// ============================================================================

/// Size of a single AGAR record, in bytes.
pub const AGAR_RECORD_SIZE: usize = 1024;

/// Agard extended header record.
///
/// Agard extended headers use 1024-byte records.  This format is used by
/// older Agard-style microscopes.  Access the raw bytes for field parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct AgarRecord {
    /// Raw 1024-byte record.
    pub raw: [u8; AGAR_RECORD_SIZE],
}

impl AgarRecord {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < AGAR_RECORD_SIZE {
            return None;
        }
        let mut raw = [0u8; AGAR_RECORD_SIZE];
        raw.copy_from_slice(&bytes[..AGAR_RECORD_SIZE]);
        Some(Self { raw })
    }
}

/// Parse extended header bytes as Agard records.
pub fn parse_agar_records(bytes: &[u8]) -> Option<Vec<AgarRecord>> {
    if bytes.is_empty() || bytes.len() % AGAR_RECORD_SIZE != 0 {
        return None;
    }
    let count = bytes.len() / AGAR_RECORD_SIZE;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * AGAR_RECORD_SIZE;
        records.push(AgarRecord::from_bytes(
            &bytes[start..start + AGAR_RECORD_SIZE],
        )?);
    }
    Some(records)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ccp4_roundtrip() {
        let mut raw = [b'X'; CCP4_RECORD_SIZE];
        raw[0] = b'A';
        let r = Ccp4Record::from_bytes(&raw).unwrap();
        assert_eq!(r.raw[0], b'A');
        assert_eq!(r.raw[CCP4_RECORD_SIZE - 1], b'X');
    }

    #[test]
    fn ccp4_as_str() {
        let mut raw = [b' '; CCP4_RECORD_SIZE];
        raw[..5].copy_from_slice(b"X,Y,Z");
        let r = Ccp4Record { raw };
        assert_eq!(r.as_str(), "X,Y,Z");
    }

    #[test]
    fn ccp4_multiple_records() {
        let mut buf = vec![0u8; CCP4_RECORD_SIZE * 3];
        buf[0] = b'1';
        buf[CCP4_RECORD_SIZE] = b'2';
        buf[CCP4_RECORD_SIZE * 2] = b'3';
        let records = parse_ccp4_records(&buf).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].raw[0], b'1');
        assert_eq!(records[1].raw[0], b'2');
        assert_eq!(records[2].raw[0], b'3');
    }

    #[test]
    fn ccp4_empty_bytes() {
        assert!(parse_ccp4_records(&[]).is_none());
    }

    #[test]
    fn ccp4_misaligned() {
        let buf = vec![0u8; CCP4_RECORD_SIZE + 1];
        assert!(parse_ccp4_records(&buf).is_none());
    }

    #[test]
    fn mrco_roundtrip() {
        let buf = vec![42u8; MRCO_RECORD_SIZE];
        let records = parse_mrco_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].raw[0], 42);
    }

    #[test]
    fn seri_roundtrip() {
        let mut buf = vec![0u8; SERI_RECORD_SIZE];
        // Set tilt angle at bytes 0-3 (f32 LE)
        buf[0..4].copy_from_slice(&(-35.5f32).to_le_bytes());
        buf[4] = 99; // marker for raw check
        let records = parse_seri_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        assert!((records[0].alpha_tilt - (-35.5)).abs() < 1e-6);
        assert_eq!(records[0].raw[4], 99);
    }

    #[test]
    fn agar_roundtrip() {
        let buf = vec![77u8; AGAR_RECORD_SIZE];
        let records = parse_agar_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].raw[0], 77);
    }

    #[test]
    fn all_parsers_reject_short_buffer() {
        assert!(parse_ccp4_records(&[0u8; 10]).is_none());
        assert!(parse_mrco_records(&[0u8; 10]).is_none());
        assert!(parse_seri_records(&[0u8; 10]).is_none());
        assert!(parse_agar_records(&[0u8; 10]).is_none());
    }

    #[test]
    fn all_parsers_reject_empty() {
        assert!(parse_ccp4_records(&[]).is_none());
        assert!(parse_mrco_records(&[]).is_none());
        assert!(parse_seri_records(&[]).is_none());
        assert!(parse_agar_records(&[]).is_none());
    }
}
