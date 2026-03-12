//! Endianness handling for MRC files

use core::fmt;

/// File endianness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileEndian {
    /// Little-endian byte order
    Little,
    /// Big-endian byte order
    Big,
}

impl FileEndian {
    /// Get the native system endianness
    #[inline]
    pub const fn native() -> Self {
        #[cfg(target_endian = "little")]
        {
            Self::Little
        }
        #[cfg(target_endian = "big")]
        {
            Self::Big
        }
    }
    
    /// Check if this is the native endianness
    #[inline]
    pub const fn is_native(self) -> bool {
        matches!(
            (self, Self::native()),
            (Self::Little, Self::Little) | (Self::Big, Self::Big)
        )
    }
    
    /// Detect endianness from MACHST machine stamp
    ///
    /// MRC2014 spec:
    /// - `0x44 0x44 0x00 0x00` = little-endian
    /// - `0x11 0x11 0x00 0x00` = big-endian
    ///
    /// Returns `None` for unrecognized values.
    #[inline]
    pub fn from_machst(machst: &[u8; 4]) -> Self {
        if machst[0] == 0x44 && machst[1] == 0x44 {
            Self::Little
        } else if machst[0] == 0x11 && machst[1] == 0x11 {
            Self::Big
        } else {
            // Default to little-endian (most common)
            Self::Little
        }
    }
    
    /// Convert to MACHST bytes
    #[inline]
    pub const fn to_machst(self) -> [u8; 4] {
        match self {
            Self::Little => [0x44, 0x44, 0x00, 0x00],
            Self::Big => [0x11, 0x11, 0x00, 0x00],
        }
    }
}

impl fmt::Display for FileEndian {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Little => write!(f, "little-endian"),
            Self::Big => write!(f, "big-endian"),
        }
    }
}
