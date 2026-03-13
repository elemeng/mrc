/// MRC data type mode
///
/// Each mode corresponds to a specific voxel type.
/// Modes are written to the header as i32 values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Mode {
    /// 8-bit signed integer
    Int8 = 0,
    /// 16-bit signed integer
    Int16 = 1,
    /// 32-bit float (most common)
    Float32 = 2,
    /// Complex with 16-bit integer components
    Int16Complex = 3,
    /// Complex with 32-bit float components
    Float32Complex = 4,
    /// 16-bit unsigned integer
    Uint16 = 6,
    /// 16-bit float (requires f16 feature)
    Float16 = 12,
    /// 4-bit packed data (experimental)
    Packed4Bit = 101,
}

/// Error for invalid mode values
///
/// This type is returned by [`Mode::try_from`] when the input value
/// doesn't correspond to a valid MRC mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidMode(pub i32);

impl core::fmt::Display for InvalidMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid MRC mode: {}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidMode {}

impl Mode {
    /// Convert from i32 mode value
    #[inline]
    pub fn from_i32(mode: i32) -> Option<Self> {
        match mode {
            0 => Some(Self::Int8),
            1 => Some(Self::Int16),
            2 => Some(Self::Float32),
            3 => Some(Self::Int16Complex),
            4 => Some(Self::Float32Complex),
            6 => Some(Self::Uint16),
            12 => Some(Self::Float16),
            101 => Some(Self::Packed4Bit),
            _ => None,
        }
    }

    /// Size of one voxel in bytes
    #[inline]
    pub const fn byte_size(self) -> usize {
        match self {
            Self::Int8 => 1,
            Self::Int16 => 2,
            Self::Float32 => 4,
            Self::Int16Complex => 4,
            Self::Float32Complex => 8,
            Self::Uint16 => 2,
            Self::Float16 => 2,
            Self::Packed4Bit => 1,
        }
    }

    /// Check if this is a complex type
    #[inline]
    pub const fn is_complex(self) -> bool {
        matches!(self, Self::Int16Complex | Self::Float32Complex)
    }

    /// Check if this is an integer type
    #[inline]
    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            Self::Int8 | Self::Int16 | Self::Int16Complex | Self::Uint16 | Self::Packed4Bit
        )
    }

    /// Check if this is a float type
    #[inline]
    pub const fn is_float(self) -> bool {
        matches!(self, Self::Float32 | Self::Float32Complex | Self::Float16)
    }

    /// Check if this mode is supported with current feature flags
    #[inline]
    pub fn is_supported(self) -> bool {
        match self {
            Self::Float16 => cfg!(feature = "f16"),
            _ => true,
        }
    }
}

impl TryFrom<i32> for Mode {
    type Error = InvalidMode;

    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_i32(value).ok_or(InvalidMode(value))
    }
}

impl From<Mode> for i32 {
    #[inline]
    fn from(mode: Mode) -> Self {
        mode as i32
    }
}
