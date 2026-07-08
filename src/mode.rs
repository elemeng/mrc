//! Voxel mode definitions and the [`Voxel`] trait.
//!
//! The MRC format stores voxel data in one of several numeric modes.
//! The [`Mode`] enum maps mode constants to their Rust representations,
//! and the [`Voxel`] trait connects Rust types to their corresponding modes
//! at compile time for type-safe I/O.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Strategy for converting complex numbers to real values.
///
/// # Example
///
/// ```rust
/// use mrc::ComplexToRealStrategy;
///
/// let s = ComplexToRealStrategy::Magnitude;
/// assert!(matches!(s, ComplexToRealStrategy::Magnitude));
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComplexToRealStrategy {
    /// Use the real component as the output value.
    RealPart,
    /// Use the imaginary component as the output value.
    ImaginaryPart,
    /// Compute `sqrt(real² + imag²)`.
    Magnitude,
    /// Compute `atan2(imag, real)`.
    Phase,
}

/// Interpretation of Mode 0 (8-bit) data for legacy files.
///
/// Some MRC files store unsigned 8-bit data under Mode 0 (which normally
/// represents `i8`). Use this enum to select the correct interpretation
/// when reading such files.
///
/// # Example
///
/// ```rust
/// use mrc::M0Interpretation;
///
/// let interp = M0Interpretation::Unsigned;
/// assert!(matches!(interp, M0Interpretation::Unsigned));
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum M0Interpretation {
    /// Treat bytes as signed `i8` values (standard Mode 0).
    Signed,
    /// Treat bytes as unsigned `u8` values (legacy convention).
    Unsigned,
}

/// MRC data mode defining the on-disk representation of voxel values.
///
/// # Example
///
/// ```rust
/// use mrc::Mode;
///
/// let mode = Mode::Float32;
/// assert_eq!(mode.byte_size(), 4);
/// assert!(mode.is_float());
/// assert!(!mode.is_integer());
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    /// 4-bit data packed two values per byte (Mode 101).
    ///
    /// Each byte stores two 4-bit nibbles: low nibble = first pixel,
    /// high nibble = second pixel. Read via [`slices_u8`](crate::Reader::slices_u8)
    /// or [`convert::<f32>()`](crate::Reader::convert); write via
    /// [`write_u4_block`](crate::Writer::write_u4_block).
    Packed4Bit = 101,
}

impl Mode {
    /// Return the MRC mode constant as an `i32` value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert_eq!(Mode::Int8.as_i32(), 0);
    /// assert_eq!(Mode::Float32.as_i32(), 2);
    /// ```
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Convert an MRC mode integer to a [`Mode`] enum value.
    ///
    /// Returns `None` for unrecognized mode values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert_eq!(Mode::from_i32(2), Some(Mode::Float32));
    /// assert_eq!(Mode::from_i32(99), None);
    /// ```
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

    /// Number of bytes required to store one voxel in this mode.
    ///
    /// For [`Packed4Bit`](Mode::Packed4Bit) this returns `1` (two voxels per byte);
    /// use [`byte_size_for_count`](Mode::byte_size_for_count) for per-voxel sizing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert_eq!(Mode::Int16.byte_size(), 2);
    /// assert_eq!(Mode::Float32Complex.byte_size(), 8);
    /// ```
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

    /// Returns `true` if this mode stores complex numbers (real + imaginary components).
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert!(Mode::Int16Complex.is_complex());
    /// assert!(!Mode::Float32.is_complex());
    /// ```
    #[inline]
    pub fn is_complex(&self) -> bool {
        matches!(self, Self::Int16Complex | Self::Float32Complex)
    }

    /// Returns `true` if this mode stores integer-valued data (including complex integers).
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert!(Mode::Uint16.is_integer());
    /// assert!(!Mode::Float32.is_integer());
    /// ```
    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::Int8 | Self::Int16 | Self::Int16Complex | Self::Uint16 | Self::Packed4Bit
        )
    }

    /// Returns `true` if this mode stores floating-point data (including complex float).
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert!(Mode::Float16.is_float());
    /// assert!(!Mode::Int8.is_float());
    /// ```
    #[inline]
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float32 | Self::Float32Complex | Self::Float16)
    }

    /// Byte size for a given number of voxels.
    ///
    /// For most modes this is `n * byte_size()`, but `Packed4Bit`
    /// stores two voxels per byte so the result is `n.div_ceil(2)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::Mode;
    ///
    /// assert_eq!(Mode::Int16.byte_size_for_count(3), 6);
    /// assert_eq!(Mode::Packed4Bit.byte_size_for_count(3), 2);
    /// ```
    #[inline]
    pub fn byte_size_for_count(&self, n: usize) -> usize {
        match self {
            Self::Packed4Bit => n.div_ceil(2),
            _ => n * self.byte_size(),
        }
    }
}

/// A complex number with 16-bit signed integer real and imaginary components.
///
/// Corresponds to MRC Mode 3. The byte layout is `[real i16, imag i16]`
/// (4 bytes total), stored in file byte order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Int16Complex {
    /// Real component.
    pub real: i16,
    /// Imaginary component.
    pub imag: i16,
}

impl Int16Complex {
    /// Convert this complex number to a real value using the given strategy.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::{Int16Complex, ComplexToRealStrategy};
    ///
    /// let c = Int16Complex { real: 3, imag: 4 };
    /// assert_eq!(c.to_real(ComplexToRealStrategy::RealPart), 3.0);
    /// ```
    #[inline]
    pub fn to_real(&self, strategy: ComplexToRealStrategy) -> f32 {
        match strategy {
            ComplexToRealStrategy::RealPart => self.real as f32,
            ComplexToRealStrategy::ImaginaryPart => self.imag as f32,
            ComplexToRealStrategy::Magnitude => {
                let r = self.real as f32;
                let i = self.imag as f32;
                (r * r + i * i).sqrt()
            }
            ComplexToRealStrategy::Phase => (self.imag as f32).atan2(self.real as f32),
        }
    }
}

/// A complex number with 32-bit float real and imaginary components.
///
/// Corresponds to MRC Mode 4. The byte layout is `[real f32, imag f32]`
/// (8 bytes total), stored in file byte order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Float32Complex {
    /// Real component.
    pub real: f32,
    /// Imaginary component.
    pub imag: f32,
}

impl Float32Complex {
    /// Convert this complex number to a real value using the given strategy.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::{Float32Complex, ComplexToRealStrategy};
    ///
    /// let c = Float32Complex { real: 3.0, imag: 4.0 };
    /// let mag = c.to_real(ComplexToRealStrategy::Magnitude);
    /// assert!((mag - 5.0).abs() < 1e-6);
    /// ```
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
