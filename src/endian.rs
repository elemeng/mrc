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
    pub fn from_machst(machst: &[u8; 4]) -> Option<Self> {
        match (machst[0], machst[1]) {
            (0x44, 0x44) => Some(Self::Little),
            (0x11, 0x11) => Some(Self::Big),
            _ => None,
        }
    }
    
    /// Detect endianness from MACHST, defaulting to little-endian with warning
    ///
    /// Use this when you want a reasonable default for unrecognized values.
    /// Returns `(endianness, true)` if detected, `(Little, false)` if defaulted.
    #[inline]
    pub fn from_machst_or_little(machst: &[u8; 4]) -> (Self, bool) {
        match Self::from_machst(machst) {
            Some(endian) => (endian, true),
            None => (Self::Little, false),
        }
    }
    
    /// Detect endianness from MACHST, returning native endianness as fallback
    ///
    /// Use this when you want a reasonable default for unrecognized values.
    #[inline]
    #[deprecated(since = "0.2.0", note = "Use from_machst_or_little instead for explicit little-endian default")]
    pub fn from_machst_or_native(machst: &[u8; 4]) -> Self {
        Self::from_machst(machst).unwrap_or_else(Self::native)
    }
    
    /// Convert to MACHST bytes
    #[inline]
    pub const fn to_machst(self) -> [u8; 4] {
        match self {
            Self::Little => [0x44, 0x44, 0x00, 0x00],
            Self::Big => [0x11, 0x11, 0x00, 0x00],
        }
    }
    
    /// Convert an i32 from file endianness to native
    #[inline]
    pub fn convert_i32_to_native(self, value: i32) -> i32 {
        match self {
            Self::Little => i32::from_le(value.to_le()),
            Self::Big => i32::from_be(value.to_be()),
        }
    }
    
    /// Convert an i32 from native to file endianness
    #[inline]
    pub fn convert_i32_from_native(self, value: i32) -> i32 {
        match self {
            Self::Little => value.to_le(),
            Self::Big => value.to_be(),
        }
    }
    
    /// Convert an f32 from file endianness to native
    #[inline]
    pub fn convert_f32_to_native(self, value: f32) -> f32 {
        let bits = value.to_bits();
        let converted = match self {
            Self::Little => u32::from_le(bits.to_le()),
            Self::Big => u32::from_be(bits.to_be()),
        };
        f32::from_bits(converted)
    }
    
    /// Convert an f32 from native to file endianness
    #[inline]
    pub fn convert_f32_from_native(self, value: f32) -> f32 {
        let bits = value.to_bits();
        let converted = match self {
            Self::Little => bits.to_le(),
            Self::Big => bits.to_be(),
        };
        f32::from_bits(converted)
    }
    
    /// Convert an i16 from file endianness to native
    #[inline]
    pub fn convert_i16_to_native(self, value: i16) -> i16 {
        match self {
            Self::Little => i16::from_le(value.to_le()),
            Self::Big => i16::from_be(value.to_be()),
        }
    }
    
    /// Convert an i16 from native to file endianness
    #[inline]
    pub fn convert_i16_from_native(self, value: i16) -> i16 {
        match self {
            Self::Little => value.to_le(),
            Self::Big => value.to_be(),
        }
    }
    
    /// Convert a u16 from file endianness to native
    #[inline]
    pub fn convert_u16_to_native(self, value: u16) -> u16 {
        match self {
            Self::Little => u16::from_le(value.to_le()),
            Self::Big => u16::from_be(value.to_be()),
        }
    }
    
    /// Convert a u16 from native to file endianness
    #[inline]
    pub fn convert_u16_from_native(self, value: u16) -> u16 {
        match self {
            Self::Little => value.to_le(),
            Self::Big => value.to_be(),
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_from_machst() {
        assert_eq!(FileEndian::from_machst(&[0x44, 0x44, 0x00, 0x00]), Some(FileEndian::Little));
        assert_eq!(FileEndian::from_machst(&[0x11, 0x11, 0x00, 0x00]), Some(FileEndian::Big));
        assert_eq!(FileEndian::from_machst(&[0x00, 0x00, 0x00, 0x00]), None);
        assert_eq!(FileEndian::from_machst(&[0xFF, 0xFF, 0x00, 0x00]), None);
    }
    
    #[test]
    fn test_to_machst() {
        assert_eq!(FileEndian::Little.to_machst(), [0x44, 0x44, 0x00, 0x00]);
        assert_eq!(FileEndian::Big.to_machst(), [0x11, 0x11, 0x00, 0x00]);
    }
    
    #[test]
    fn test_is_native() {
        let native = FileEndian::native();
        assert!(native.is_native());
    }
}