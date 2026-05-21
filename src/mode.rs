//! Voxel mode definitions and the [`Voxel`] trait.
//!
//! The MRC format stores voxel data in one of several numeric modes.
//! The [`Mode`] enum maps mode constants to their Rust representations,
//! and the [`Voxel`] trait connects Rust types to their corresponding modes
//! at compile time for type-safe I/O.

/// Strategy for converting complex numbers to real values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComplexToRealStrategy {
    RealPart,
    ImaginaryPart,
    Magnitude,
    Phase,
}

/// Interpretation of Mode 0 (8-bit) data for legacy files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum M0Interpretation {
    Signed,
    Unsigned,
}

/// MRC data mode defining the on-disk representation of voxel values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// Signed 8-bit integer (Mode 0).
    Int8 = 0,
    /// Signed 16-bit integer (Mode 1).
    Int16 = 1,
    /// 32-bit floating point (Mode 2).
    Float32 = 2,
    /// Complex number with 16-bit integer components (Mode 3)
    ///
    /// # Byte Order
    /// The layout is `[real i16 (2 bytes), imag i16 (2 bytes)]` which matches the
    /// de facto standard used by CCP4, IMOD, and other MRC implementations.
    /// This is not explicitly specified in MRC2014 but is universally adopted.
    Int16Complex = 3,
    /// Complex number with 32-bit float components (Mode 4)
    ///
    /// # Byte Order
    /// The layout is `[real f32 (4 bytes), imag f32 (4 bytes)]` which matches the
    /// de facto standard used by CCP4, IMOD, and other MRC implementations.
    /// This is not explicitly specified in MRC2014 but is universally adopted.
    Float32Complex = 4,
    /// Unsigned 16-bit integer (Mode 6).
    Uint16 = 6,
    /// 16-bit floating point (Mode 12).
    Float16 = 12,
    /// 4-bit data packed two values per byte (mode 101)
    Packed4Bit = 101,
}

impl Mode {
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

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

    #[inline]
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Int8 => 1,
            Self::Int16 => 2,
            Self::Float32 => 4,
            Self::Int16Complex => 4,   // 2 bytes real + 2 bytes imaginary
            Self::Float32Complex => 8, // 4 bytes real + 4 bytes imaginary
            Self::Uint16 => 2,
            Self::Float16 => 2,
            Self::Packed4Bit => 1, // 4 bits per value, 2 values per byte
        }
    }

    #[inline]
    pub fn is_complex(&self) -> bool {
        matches!(self, Self::Int16Complex | Self::Float32Complex)
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::Int8 | Self::Int16 | Self::Int16Complex | Self::Uint16 | Self::Packed4Bit
        )
    }

    #[inline]
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float32 | Self::Float32Complex | Self::Float16)
    }

    /// Byte size for a given number of voxels.
    ///
    /// For most modes this is `n * byte_size()`, but `Packed4Bit`
    /// stores two voxels per byte so the result is `n.div_ceil(2)`.
    #[inline]
    pub fn byte_size_for_count(&self, n: usize) -> usize {
        match self {
            Self::Packed4Bit => n.div_ceil(2),
            _ => n * self.byte_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Int16Complex {
    pub real: i16,
    pub imag: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Float32Complex {
    pub real: f32,
    pub imag: f32,
}

impl Default for Float32Complex {
    fn default() -> Self {
        Self {
            real: 0.0,
            imag: 0.0,
        }
    }
}

impl Float32Complex {
    /// Convert this complex number to a real value using the given strategy.
    #[inline]
    pub fn to_real(&self, strategy: ComplexToRealStrategy) -> f32 {
        match strategy {
            ComplexToRealStrategy::RealPart => self.real,
            ComplexToRealStrategy::ImaginaryPart => self.imag,
            ComplexToRealStrategy::Magnitude => {
                (self.real * self.real + self.imag * self.imag).sqrt()
            }
            ComplexToRealStrategy::Phase => self.imag.atan2(self.real),
        }
    }
}

/// 4-bit data packed two values per byte (mode 101).
///
/// # Nibble ordering
/// The MRC2014 specification does not explicitly define which nibble comes
/// first. This implementation follows the de-facto convention used by CCP4,
/// IMOD and other major packages:
///
/// * **low nibble** (`bits 0–3`) → first voxel
/// * **high nibble** (`bits 4–7`) → second voxel
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Packed4Bit(pub(crate) u8);

impl Packed4Bit {
    /// Create a new Packed4Bit value
    pub fn new(value: u8) -> Self {
        Self(value)
    }

    /// First voxel stored in the low nibble (bits 0–3).
    pub fn first(&self) -> u8 {
        self.0 & 0x0F
    }

    /// Second voxel stored in the high nibble (bits 4–7).
    pub fn second(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }
}

/// Trait for MRC voxel types with compile-time mode tracking.
///
/// Each voxel type knows its MRC mode constant, enabling type-safe I/O
/// without runtime mode dispatch.
///
/// Note: `BYTE_SIZE` is inherited from the `EndianCodec` supertrait.
pub trait Voxel:
    crate::engine::codec::EndianCodec + Copy + Send + Sync + Default + 'static
{
    /// The MRC mode constant for this voxel type
    const MODE: Mode;
}

impl Voxel for i8 {
    const MODE: Mode = Mode::Int8;
}

impl Voxel for i16 {
    const MODE: Mode = Mode::Int16;
}

impl Voxel for f32 {
    const MODE: Mode = Mode::Float32;
}

impl Voxel for Int16Complex {
    const MODE: Mode = Mode::Int16Complex;
}

impl Voxel for Float32Complex {
    const MODE: Mode = Mode::Float32Complex;
}

impl Voxel for u16 {
    const MODE: Mode = Mode::Uint16;
}

#[cfg(feature = "f16")]
impl Voxel for crate::f16 {
    const MODE: Mode = Mode::Float16;
}

// Note: Packed4Bit does not implement Voxel or EndianCodec because full
// read/write support for 4-bit packed data is not yet implemented.
// The Packed4Bit type is provided for manual unpacking via first()/second().
