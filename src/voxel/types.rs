//! Voxel trait hierarchy and concrete types for MRC data
//!
//! This module provides:
//! - `Voxel`: Base trait for all voxel types
//! - `ScalarVoxel`: Marker for scalar types
//! - `RealVoxel`: Marker for floating-point types
//! - `IntegerVoxel`: Marker for integer types
//! - Concrete types: ComplexI16, ComplexF32, Packed4Bit

extern crate alloc;

use crate::core::Mode;
use bytemuck::{Pod, Zeroable};

/// Base trait for all voxel types
///
/// This trait is sealed - only types defined in this crate can implement it.
pub trait Voxel: Copy + Send + Sync + 'static + private::Sealed {
    /// The MRC mode for this voxel type
    const MODE: Mode;
}

mod private {
    pub trait Sealed {}
}

// Implement Sealed for primitive types
impl private::Sealed for i8 {}
impl private::Sealed for i16 {}
impl private::Sealed for u16 {}
impl private::Sealed for f32 {}
#[cfg(feature = "f16")]
impl private::Sealed for half::f16 {}

/// Marker for scalar (non-complex) voxel types
pub trait ScalarVoxel: Voxel {}

impl ScalarVoxel for i8 {}
impl ScalarVoxel for i16 {}
impl ScalarVoxel for u16 {}
impl ScalarVoxel for f32 {}
#[cfg(feature = "f16")]
impl ScalarVoxel for half::f16 {}

/// Marker for real (floating-point) voxel types
pub trait RealVoxel: ScalarVoxel {
    /// Convert from f32
    fn from_f32(v: f32) -> Self;
    /// Convert to f32
    fn to_f32(self) -> f32;
}

impl RealVoxel for f32 {
    #[inline]
    fn from_f32(v: f32) -> Self {
        v
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }
}

#[cfg(feature = "f16")]
impl RealVoxel for half::f16 {
    #[inline]
    fn from_f32(v: f32) -> Self {
        half::f16::from_f32(v)
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self.to_f32()
    }
}

impl RealVoxel for i16 {
    #[inline]
    fn from_f32(v: f32) -> Self {
        v as i16
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
}

impl RealVoxel for i8 {
    #[inline]
    fn from_f32(v: f32) -> Self {
        v as i8
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
}

impl RealVoxel for u16 {
    #[inline]
    fn from_f32(v: f32) -> Self {
        v as u16
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
}

/// Marker for integer voxel types
pub trait IntegerVoxel: ScalarVoxel {
    /// Convert from i64 with saturation
    fn from_i64(v: i64) -> Self;
    /// Convert to i64
    fn to_i64(self) -> i64;
    /// Convert from u64 with saturation
    fn from_u64(v: u64) -> Self;
    /// Convert to u64
    fn to_u64(self) -> u64;
}

impl IntegerVoxel for i8 {
    #[inline]
    fn from_i64(v: i64) -> Self {
        v as i8
    }
    #[inline]
    fn to_i64(self) -> i64 {
        self as i64
    }
    #[inline]
    fn from_u64(v: u64) -> Self {
        v as i8
    }
    #[inline]
    fn to_u64(self) -> u64 {
        self as u8 as u64
    }
}

impl IntegerVoxel for i16 {
    #[inline]
    fn from_i64(v: i64) -> Self {
        v as i16
    }
    #[inline]
    fn to_i64(self) -> i64 {
        self as i64
    }
    #[inline]
    fn from_u64(v: u64) -> Self {
        v as i16
    }
    #[inline]
    fn to_u64(self) -> u64 {
        self as u16 as u64
    }
}

impl IntegerVoxel for u16 {
    #[inline]
    fn from_i64(v: i64) -> Self {
        v as u16
    }
    #[inline]
    fn to_i64(self) -> i64 {
        self as i64
    }
    #[inline]
    fn from_u64(v: u64) -> Self {
        v as u16
    }
    #[inline]
    fn to_u64(self) -> u64 {
        self as u64
    }
}

// ============================================================================
// Complex types
// ============================================================================

/// Complex number with i16 components (Mode 3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct ComplexI16 {
    pub re: i16,
    pub im: i16,
}

impl ComplexI16 {
    /// Create a new complex number
    #[inline]
    pub const fn new(re: i16, im: i16) -> Self {
        Self { re, im }
    }

    /// Create a complex number from real part only (imaginary part is 0)
    #[inline]
    pub const fn from_real(re: i16) -> Self {
        Self { re, im: 0 }
    }

    /// Returns the square of the magnitude (re² + im²)
    #[inline]
    pub fn magnitude_squared(self) -> i32 {
        let re = self.re as i32;
        let im = self.im as i32;
        re * re + im * im
    }

    /// Returns the magnitude (sqrt(re² + im²)) as f32
    #[inline]
    #[cfg(feature = "std")]
    pub fn magnitude(self) -> f32 {
        (self.magnitude_squared() as f32).sqrt()
    }

    /// Returns the phase angle in radians (atan2(im, re))
    #[inline]
    #[cfg(feature = "std")]
    pub fn phase(self) -> f32 {
        (self.im as f32).atan2(self.re as f32)
    }

    /// Returns the complex conjugate (re - im*i)
    #[inline]
    pub const fn conjugate(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    /// Convert to ComplexF32
    #[inline]
    pub fn to_complex_f32(self) -> ComplexF32 {
        ComplexF32 {
            re: self.re as f32,
            im: self.im as f32,
        }
    }
}

impl core::ops::Add for ComplexI16 {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re.saturating_add(other.re),
            im: self.im.saturating_add(other.im),
        }
    }
}

impl core::ops::Sub for ComplexI16 {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            re: self.re.saturating_sub(other.re),
            im: self.im.saturating_sub(other.im),
        }
    }
}

impl core::ops::Mul for ComplexI16 {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        let a = self.re as i32;
        let b = self.im as i32;
        let c = other.re as i32;
        let d = other.im as i32;
        Self {
            re: (a * c - b * d) as i16,
            im: (a * d + b * c) as i16,
        }
    }
}

impl core::ops::Neg for ComplexI16 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            re: self.re.wrapping_neg(),
            im: self.im.wrapping_neg(),
        }
    }
}

/// Complex number with f32 components (Mode 4)
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct ComplexF32 {
    pub re: f32,
    pub im: f32,
}

impl ComplexF32 {
    /// Create a new complex number
    #[inline]
    pub const fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    /// Create a complex number from real part only
    #[inline]
    pub const fn from_real(re: f32) -> Self {
        Self { re, im: 0.0 }
    }

    /// Returns the square of the magnitude
    #[inline]
    pub fn magnitude_squared(self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    /// Returns the magnitude
    #[inline]
    #[cfg(feature = "std")]
    pub fn magnitude(self) -> f32 {
        self.magnitude_squared().sqrt()
    }

    /// Returns the phase angle in radians
    #[inline]
    #[cfg(feature = "std")]
    pub fn phase(self) -> f32 {
        self.im.atan2(self.re)
    }

    /// Returns the complex conjugate
    #[inline]
    pub const fn conjugate(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    /// Returns the reciprocal
    #[inline]
    pub fn recip(self) -> Self {
        let magsq = self.magnitude_squared();
        Self {
            re: self.re / magsq,
            im: -self.im / magsq,
        }
    }

    /// Normalize to unit magnitude
    #[inline]
    #[cfg(feature = "std")]
    pub fn normalize(self) -> Self {
        let mag = self.magnitude();
        if mag > 0.0 {
            Self {
                re: self.re / mag,
                im: self.im / mag,
            }
        } else {
            Self { re: 0.0, im: 0.0 }
        }
    }

    /// Convert to ComplexI16 with optional scaling
    #[inline]
    pub fn to_complex_i16(self, scale: f32) -> ComplexI16 {
        ComplexI16 {
            re: (self.re * scale) as i16,
            im: (self.im * scale) as i16,
        }
    }
}

impl core::ops::Add for ComplexF32 {
    type Output = Self;
    #[inline]
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

impl core::ops::Sub for ComplexF32 {
    type Output = Self;
    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }
}

impl core::ops::Mul for ComplexF32 {
    type Output = Self;
    #[inline]
    fn mul(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

impl core::ops::Div for ComplexF32 {
    type Output = Self;
    #[inline]
    fn div(self, other: Self) -> Self {
        let magsq = other.magnitude_squared();
        Self {
            re: (self.re * other.re + self.im * other.im) / magsq,
            im: (self.im * other.re - self.re * other.im) / magsq,
        }
    }
}

impl core::ops::Neg for ComplexF32 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

impl core::ops::Mul<f32> for ComplexF32 {
    type Output = Self;
    #[inline]
    fn mul(self, scalar: f32) -> Self {
        Self {
            re: self.re * scalar,
            im: self.im * scalar,
        }
    }
}

impl core::ops::Div<f32> for ComplexF32 {
    type Output = Self;
    #[inline]
    fn div(self, scalar: f32) -> Self {
        Self {
            re: self.re / scalar,
            im: self.im / scalar,
        }
    }
}

// Implement Sealed for complex types
impl private::Sealed for ComplexI16 {}
impl private::Sealed for ComplexF32 {}

// Implement Voxel for complex types
impl Voxel for ComplexI16 {
    const MODE: Mode = Mode::Int16Complex;
}

impl Voxel for ComplexF32 {
    const MODE: Mode = Mode::Float32Complex;
}

/// Marker for complex voxel types
pub trait ComplexVoxel: Voxel {
    /// The real component type
    type Real: ScalarVoxel;
}

impl ComplexVoxel for ComplexI16 {
    type Real = i16;
}

impl ComplexVoxel for ComplexF32 {
    type Real = f32;
}

// ============================================================================
// Voxel implementations for primitive types
// ============================================================================

impl Voxel for i8 {
    const MODE: Mode = Mode::Int8;
}

impl Voxel for i16 {
    const MODE: Mode = Mode::Int16;
}

impl Voxel for u16 {
    const MODE: Mode = Mode::Uint16;
}

impl Voxel for f32 {
    const MODE: Mode = Mode::Float32;
}

#[cfg(feature = "f16")]
impl Voxel for half::f16 {
    const MODE: Mode = Mode::Float16;
}

/// Legacy type alias
pub type Int16Complex = ComplexI16;

/// Legacy type alias
pub type Float32Complex = ComplexF32;

// ============================================================================
// Packed 4-bit type (Mode 101)
// ============================================================================

/// Packed 4-bit values (Mode 101)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packed4Bit {
    pub byte: u8,
}

impl Packed4Bit {
    /// Create from raw byte
    #[inline]
    pub const fn new(byte: u8) -> Self {
        Self { byte }
    }

    /// Create from two values
    #[inline]
    pub fn from_values(first: u8, second: u8) -> Self {
        assert!(first <= 15, "First value must be 0-15");
        assert!(second <= 15, "Second value must be 0-15");
        Self {
            byte: first | (second << 4),
        }
    }

    /// Create from two values with saturation
    #[inline]
    pub fn from_values_saturated(first: u8, second: u8) -> Self {
        let first = first.min(15);
        let second = second.min(15);
        Self {
            byte: first | (second << 4),
        }
    }

    /// Get the first (lower) 4-bit value
    #[inline]
    pub const fn first(&self) -> u8 {
        self.byte & 0x0F
    }

    /// Get the second (upper) 4-bit value
    #[inline]
    pub const fn second(&self) -> u8 {
        (self.byte >> 4) & 0x0F
    }

    /// Get both values as a tuple
    #[inline]
    pub const fn values(&self) -> (u8, u8) {
        (self.first(), self.second())
    }

    /// Get both values as an array
    #[inline]
    pub const fn unpack(&self) -> [u8; 2] {
        [self.first(), self.second()]
    }

    /// Set the first value
    #[inline]
    pub fn set_first(&mut self, value: u8) {
        assert!(value <= 15, "Value must be 0-15");
        self.byte = (self.byte & 0xF0) | value;
    }

    /// Set the second value
    #[inline]
    pub fn set_second(&mut self, value: u8) {
        assert!(value <= 15, "Value must be 0-15");
        self.byte = (self.byte & 0x0F) | (value << 4);
    }

    /// Set both values
    #[inline]
    pub fn set_values(&mut self, first: u8, second: u8) {
        assert!(first <= 15, "First value must be 0-15");
        assert!(second <= 15, "Second value must be 0-15");
        self.byte = first | (second << 4);
    }

    /// Check if both values are valid
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.first() <= 15 && self.second() <= 15
    }

    /// Get the nth value
    #[inline]
    pub const fn get(&self, index: usize) -> Option<u8> {
        match index {
            0 => Some(self.first()),
            1 => Some(self.second()),
            _ => None,
        }
    }

    /// Sum of both values
    #[inline]
    pub const fn sum(&self) -> u8 {
        self.first() + self.second()
    }

    /// Maximum of both values
    #[inline]
    pub const fn max(&self) -> u8 {
        let first = self.first();
        let second = self.second();
        if first > second { first } else { second }
    }

    /// Minimum of both values
    #[inline]
    pub const fn min(&self) -> u8 {
        let first = self.first();
        let second = self.second();
        if first < second { first } else { second }
    }
}

impl IntoIterator for Packed4Bit {
    type Item = u8;
    type IntoIter = core::array::IntoIter<u8, 2>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.unpack().into_iter()
    }
}

impl private::Sealed for Packed4Bit {}

impl Voxel for Packed4Bit {
    const MODE: Mode = Mode::Packed4Bit;
}

impl ScalarVoxel for Packed4Bit {}
