//! Endianness handling for MRC files

/// Endianness of MRC file data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEndian {
    LittleEndian,
    BigEndian,
}

impl FileEndian {
    /// Detect file endianness from MACHST machine stamp
    pub fn from_machst(machst: &[u8; 4]) -> Self {
        let endian = if machst[0] == 0x44 && machst[1] == 0x44 {
            FileEndian::LittleEndian
        } else if machst[0] == 0x11 && machst[1] == 0x11 {
            FileEndian::BigEndian
        } else {
            FileEndian::LittleEndian
        };

        #[cfg(feature = "std")]
        {
            if machst[2] != 0 || machst[3] != 0 {
                std::eprintln!(
                    "Warning: Non-standard MACHST padding bytes: {:02X} {:02X} {:02X} {:02X}",
                    machst[0], machst[1], machst[2], machst[3]
                );
            }
        }

        endian
    }

    pub fn to_machst(self) -> [u8; 4] {
        match self {
            FileEndian::LittleEndian => [0x44, 0x44, 0x00, 0x00],
            FileEndian::BigEndian => [0x11, 0x11, 0x00, 0x00],
        }
    }

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

    #[inline]
    pub fn is_native(self) -> bool {
        self == Self::native()
    }
}