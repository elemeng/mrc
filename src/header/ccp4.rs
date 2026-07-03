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
        let records = super::parse_ccp4_records(&buf).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].raw[0], b'1');
        assert_eq!(records[1].raw[0], b'2');
        assert_eq!(records[2].raw[0], b'3');
    }

    #[test]
    fn ccp4_empty_bytes() {
        assert!(super::parse_ccp4_records(&[]).is_none());
    }

    #[test]
    fn ccp4_misaligned() {
        let buf = vec![0u8; CCP4_RECORD_SIZE + 1];
        assert!(super::parse_ccp4_records(&buf).is_none());
    }
}
