//! Endianness handling for MRC files (internal)

use core::fmt;

/// File endianness (internal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FileEndian {
    Little,
    Big,
}

impl FileEndian {
    /// Get the native system endianness
    #[inline]
    pub(crate) const fn native() -> Self {
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
    pub(crate) const fn is_native(self) -> bool {
        matches!(
            (self, Self::native()),
            (Self::Little, Self::Little) | (Self::Big, Self::Big)
        )
    }

    /// Detect endianness from MACHST machine stamp
    #[inline]
    pub(crate) fn from_machst(machst: &[u8; 4]) -> Option<Self> {
        match (machst[0], machst[1]) {
            (0x44, 0x44) => Some(Self::Little),
            (0x11, 0x11) => Some(Self::Big),
            _ => None,
        }
    }

    /// Detect endianness from MACHST, defaulting to little-endian
    #[inline]
    pub(crate) fn from_machst_or_little(machst: &[u8; 4]) -> (Self, bool) {
        match Self::from_machst(machst) {
            Some(endian) => (endian, true),
            None => (Self::Little, false),
        }
    }

    /// Convert to MACHST bytes
    #[inline]
    pub(crate) const fn to_machst(self) -> [u8; 4] {
        match self {
            Self::Little => [0x44, 0x44, 0x00, 0x00],
            Self::Big => [0x11, 0x11, 0x00, 0x00],
        }
    }
}

/// Trait for types that can be converted between endianness (internal)
pub(crate) trait EndianConvert: Copy + Sized {
    fn convert_from_file(self, endian: FileEndian) -> Self;
}

// Macro for integer types with swap_bytes
macro_rules! impl_endian_convert_swap {
    ($type:ty) => {
        impl EndianConvert for $type {
            #[inline]
            fn convert_from_file(self, endian: FileEndian) -> Self {
                if endian.is_native() {
                    self
                } else {
                    self.swap_bytes()
                }
            }
        }
    };
}

impl_endian_convert_swap!(i16);
impl_endian_convert_swap!(u16);
impl_endian_convert_swap!(i32);
impl_endian_convert_swap!(u32);
impl_endian_convert_swap!(i64);
impl_endian_convert_swap!(u64);

impl EndianConvert for f32 {
    #[inline]
    fn convert_from_file(self, endian: FileEndian) -> Self {
        if endian.is_native() {
            self
        } else {
            f32::from_bits(self.to_bits().swap_bytes())
        }
    }
}

impl EndianConvert for f64 {
    #[inline]
    fn convert_from_file(self, endian: FileEndian) -> Self {
        if endian.is_native() {
            self
        } else {
            f64::from_bits(self.to_bits().swap_bytes())
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
        assert_eq!(
            FileEndian::from_machst(&[0x44, 0x44, 0x00, 0x00]),
            Some(FileEndian::Little)
        );
        assert_eq!(
            FileEndian::from_machst(&[0x11, 0x11, 0x00, 0x00]),
            Some(FileEndian::Big)
        );
        assert_eq!(FileEndian::from_machst(&[0x00, 0x00, 0x00, 0x00]), None);
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
