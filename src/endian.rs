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
        { Self::Little }
        #[cfg(target_endian = "big")]
        { Self::Big }
    }

    /// Check if this is the native endianness
    #[inline]
    pub const fn is_native(self) -> bool {
        matches!((self, Self::native()), (Self::Little, Self::Little) | (Self::Big, Self::Big))
    }

    /// Detect endianness from MACHST machine stamp
    #[inline]
    pub fn from_machst(machst: &[u8; 4]) -> Option<Self> {
        match (machst[0], machst[1]) {
            (0x44, 0x44) => Some(Self::Little),
            (0x11, 0x11) => Some(Self::Big),
            _ => None,
        }
    }

    /// Detect endianness from MACHST, defaulting to little-endian
    #[inline]
    pub fn from_machst_or_little(machst: &[u8; 4]) -> (Self, bool) {
        match Self::from_machst(machst) {
            Some(endian) => (endian, true),
            None => (Self::Little, false),
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

    /// Convert a value from file endianness to native
    #[inline]
    pub fn convert<T: EndianConvert>(self, value: T) -> T {
        value.convert_from_file(self)
    }
}

/// Trait for types that can be converted between endianness
pub trait EndianConvert: Copy + Sized {
    /// Convert from file endianness to native
    fn convert_from_file(self, endian: FileEndian) -> Self;
}

macro_rules! impl_endian_convert {
    ($type:ty) => {
        impl EndianConvert for $type {
            #[inline]
            fn convert_from_file(self, endian: FileEndian) -> Self {
                match endian {
                    FileEndian::Little => Self::from_le(self.to_le()),
                    FileEndian::Big => Self::from_be(self.to_be()),
                }
            }
        }
    };
}

impl_endian_convert!(i16);
impl_endian_convert!(u16);
impl_endian_convert!(i32);
impl_endian_convert!(u32);
impl_endian_convert!(i64);
impl_endian_convert!(u64);

impl EndianConvert for f32 {
    #[inline]
    fn convert_from_file(self, endian: FileEndian) -> Self {
        let bits = self.to_bits();
        let converted = match endian {
            FileEndian::Little => u32::from_le(bits.to_le()),
            FileEndian::Big => u32::from_be(bits.to_be()),
        };
        f32::from_bits(converted)
    }
}

impl EndianConvert for f64 {
    #[inline]
    fn convert_from_file(self, endian: FileEndian) -> Self {
        let bits = self.to_bits();
        let converted = match endian {
            FileEndian::Little => u64::from_le(bits.to_le()),
            FileEndian::Big => u64::from_be(bits.to_be()),
        };
        f64::from_bits(converted)
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