//! Endianness handling for MRC files

/// Endianness of MRC file data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEndian {
    LittleEndian,
    BigEndian,
}

impl FileEndian {
    /// Try to detect file endianness from MACHST machine stamp
    ///
    /// According to MRC2014 spec:
    /// - 0x44 0x44 0x00 0x00 indicates little-endian
    /// - 0x11 0x11 0x00 0x00 indicates big-endian
    ///
    /// Returns `None` if the MACHST value is not recognized.
    ///
    /// # Note
    /// Endianness is determined solely from the first two bytes of MACHST.
    /// The last two bytes (padding) are ignored for endianness detection.
    pub fn try_from_machst(machst: &[u8; 4]) -> Option<Self> {
        // Check first two bytes (bytes 213-214 in header)
        // 0x44 = 'D' in ASCII, indicates little-endian
        // 0x11 indicates big-endian
        if machst[0] == 0x44 && machst[1] == 0x44 {
            Some(FileEndian::LittleEndian)
        } else if machst[0] == 0x11 && machst[1] == 0x11 {
            Some(FileEndian::BigEndian)
        } else {
            None
        }
    }

    /// Detect file endianness from MACHST machine stamp
    ///
    /// According to MRC2014 spec:
    /// - 0x44 0x44 0x00 0x00 indicates little-endian
    /// - 0x11 0x11 0x00 0x00 indicates big-endian
    ///
    /// # Note
    /// Endianness is determined solely from the first two bytes of MACHST.
    /// The last two bytes (padding) are ignored for endianness detection,
    /// but a warning is emitted if they contain non-zero values.
    /// If the MACHST value is not recognized, defaults to little-endian.
    pub fn from_machst(machst: &[u8; 4]) -> Self {
        let endian = Self::try_from_machst(machst).unwrap_or_else(|| {
            // Default to little-endian for unknown values
            // (most common in practice)
            #[cfg(feature = "std")]
            std::eprintln!(
                "Warning: Unrecognized MACHST value: {:02X} {:02X} {:02X} {:02X}, defaulting to little-endian",
                machst[0], machst[1], machst[2], machst[3]
            );
            FileEndian::LittleEndian
        });

        // Warn about non-standard padding bytes (bytes 2-3)
        #[cfg(feature = "std")]
        {
            if machst[2] != 0 || machst[3] != 0 {
                std::eprintln!(
                    "Warning: Non-standard MACHST padding bytes: {:02X} {:02X} {:02X} {:02X}",
                    machst[0],
                    machst[1],
                    machst[2],
                    machst[3]
                );
            }
        }

        endian
    }

    /// Convert FileEndian to MACHST bytes
    ///
    /// Returns the 4-byte machine stamp encoding for this endianness.
    pub fn to_machst(self) -> [u8; 4] {
        match self {
            FileEndian::LittleEndian => [0x44, 0x44, 0x00, 0x00],
            FileEndian::BigEndian => [0x11, 0x11, 0x00, 0x00],
        }
    }

    /// Get native system endianness
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

    /// Check if this is the native endianness
    #[inline]
    pub fn is_native(self) -> bool {
        self == Self::native()
    }
}
