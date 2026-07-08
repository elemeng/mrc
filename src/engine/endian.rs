//! Endianness detection and handling for MRC files.
//!
//! MRC files encode byte order via the 4-byte MACHST machine stamp.
//! This module detects the stamp, provides the [`FileEndian`] enum, and
//! defines the [`MachstInfo`] metadata type.

/// Endianness of MRC file data.
///
/// # Examples
///
/// ```rust
/// use mrc::FileEndian;
/// let le = FileEndian::LittleEndian;
/// assert_eq!(le.to_machst(), [0x44, 0x44, 0x00, 0x00]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEndian {
    /// Little-endian byte order.
    LittleEndian,
    /// Big-endian byte order.
    BigEndian,
}

impl FileEndian {
    /// Detect file endianness from MACHST machine stamp.
    ///
    /// Recognises the standard stamps (`0x44 0x44` for little-endian,
    /// `0x11 0x11` for big-endian) as well as the CCP4 variant
    /// `0x44 0x41`. Any unknown stamp falls back to little-endian.
    /// Use [`from_machst_with_info`](Self::from_machst_with_info) to
    /// inspect whether the stamp was non-standard.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::FileEndian;
    /// let le = FileEndian::from_machst(&[0x44, 0x44, 0x00, 0x00]);
    /// assert_eq!(le, FileEndian::LittleEndian);
    /// let be = FileEndian::from_machst(&[0x11, 0x11, 0x00, 0x00]);
    /// assert_eq!(be, FileEndian::BigEndian);
    /// ```
    pub fn from_machst(machst: &[u8; 4]) -> Self {
        Self::from_machst_with_info(machst).endian
    }

    /// Detect endianness and return metadata about the stamp.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::FileEndian;
    /// let info = FileEndian::from_machst_with_info(&[0x44, 0x44, 0x00, 0x00]);
    /// assert_eq!(info.endian, FileEndian::LittleEndian);
    /// assert!(info.is_standard);
    /// ```
    pub fn from_machst_with_info(machst: &[u8; 4]) -> MachstInfo {
        if machst[0] == 0x44 && machst[1] == 0x44 {
            MachstInfo {
                endian: FileEndian::LittleEndian,
                is_standard: true,
                description: "0x44 0x44 (little-endian)",
            }
        } else if machst[0] == 0x44 && machst[1] == 0x41 {
            MachstInfo {
                endian: FileEndian::LittleEndian,
                is_standard: false,
                description: "0x44 0x41 (little-endian, CCP4 variant)",
            }
        } else if machst[0] == 0x11 && machst[1] == 0x11 {
            MachstInfo {
                endian: FileEndian::BigEndian,
                is_standard: true,
                description: "0x11 0x11 (big-endian)",
            }
        } else {
            MachstInfo {
                endian: FileEndian::LittleEndian,
                is_standard: false,
                description: "non-standard (fallback to little-endian)",
            }
        }
    }

    /// Returns the 4-byte MACHST stamp for this endianness.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::FileEndian;
    /// let stamp = FileEndian::BigEndian.to_machst();
    /// assert_eq!(stamp, [0x11, 0x11, 0x00, 0x00]);
    /// ```
    pub fn to_machst(self) -> [u8; 4] {
        match self {
            FileEndian::LittleEndian => [0x44, 0x44, 0x00, 0x00],
            FileEndian::BigEndian => [0x11, 0x11, 0x00, 0x00],
        }
    }

    /// Return the opposite endianness.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::FileEndian;
    /// assert_eq!(FileEndian::LittleEndian.opposite(), FileEndian::BigEndian);
    /// assert_eq!(FileEndian::BigEndian.opposite(), FileEndian::LittleEndian);
    /// ```
    pub fn opposite(self) -> Self {
        match self {
            FileEndian::LittleEndian => FileEndian::BigEndian,
            FileEndian::BigEndian => FileEndian::LittleEndian,
        }
    }

    /// Returns the native endianness of the host platform.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::FileEndian;
    /// let native = FileEndian::native();
    /// assert!(native == FileEndian::LittleEndian || native == FileEndian::BigEndian);
    /// ```
    #[inline]
    pub fn native() -> Self {
        #[cfg(target_endian = "little")]
        {
            FileEndian::LittleEndian
        }
        #[cfg(target_endian = "big")]
        {
            FileEndian::BigEndian
        }
    }

    /// Returns `true` if this endianness matches the host platform.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mrc::FileEndian;
    /// assert!(FileEndian::native().is_native());
    /// ```
    #[inline]
    pub fn is_native(self) -> bool {
        self == Self::native()
    }
}

/// Metadata about a MACHST machine stamp.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachstInfo {
    pub endian: FileEndian,
    pub is_standard: bool,
    pub description: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machst_le_standard() {
        let info = FileEndian::from_machst_with_info(&[0x44, 0x44, 0x00, 0x00]);
        assert_eq!(info.endian, FileEndian::LittleEndian);
        assert!(info.is_standard);
    }

    #[test]
    fn test_machst_le_ccp4_variant() {
        let info = FileEndian::from_machst_with_info(&[0x44, 0x41, 0x00, 0x00]);
        assert_eq!(info.endian, FileEndian::LittleEndian);
        assert!(!info.is_standard);
    }

    #[test]
    fn test_machst_be_standard() {
        let info = FileEndian::from_machst_with_info(&[0x11, 0x11, 0x00, 0x00]);
        assert_eq!(info.endian, FileEndian::BigEndian);
        assert!(info.is_standard);
    }

    #[test]
    fn test_machst_non_standard_fallback() {
        let info = FileEndian::from_machst_with_info(&[0xAB, 0xCD, 0x00, 0x00]);
        assert_eq!(info.endian, FileEndian::LittleEndian);
        assert!(!info.is_standard);
    }

    #[test]
    fn test_opposite() {
        assert_eq!(FileEndian::LittleEndian.opposite(), FileEndian::BigEndian);
        assert_eq!(FileEndian::BigEndian.opposite(), FileEndian::LittleEndian);
    }
}
