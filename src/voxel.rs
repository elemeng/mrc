//! Voxel trait hierarchy for MRC data types
//!
//! This module provides a type-safe foundation for voxel data:
//! - `Voxel`: Base trait for all voxel types
//! - `ScalarVoxel`: Marker for scalar (non-complex) types
//! - `RealVoxel`: Marker for real (floating-point) types  
//! - `ComplexI16`/`ComplexF32`: Complex number types

use bytemuck::{Pod, Zeroable};

/// Base trait for all voxel types
///
/// This trait is sealed - only types defined in this crate can implement it.
pub trait Voxel: Copy + Send + Sync + 'static + private::Sealed {
    /// Minimum representable value
    const MIN: Self;
    /// Maximum representable value
    const MAX: Self;
}

mod private {
    pub trait Sealed {}
}

// Implement Sealed for primitive types
impl private::Sealed for i8 {}
impl private::Sealed for i16 {}
impl private::Sealed for u16 {}
impl private::Sealed for f32 {}
impl private::Sealed for half::f16 {}

/// Marker for scalar (non-complex) voxel types
pub trait ScalarVoxel: Voxel {}

impl ScalarVoxel for i8 {}
impl ScalarVoxel for i16 {}
impl ScalarVoxel for u16 {}
impl ScalarVoxel for f32 {}
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
    fn from_f32(v: f32) -> Self { v }
    #[inline]
    fn to_f32(self) -> f32 { self }
}

impl RealVoxel for half::f16 {
    #[inline]
    fn from_f32(v: f32) -> Self { half::f16::from_f32(v) }
    #[inline]
    fn to_f32(self) -> f32 { self.to_f32() }
}

impl RealVoxel for i16 {
    #[inline]
    fn from_f32(v: f32) -> Self { v as i16 }
    #[inline]
    fn to_f32(self) -> f32 { self as f32 }
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
}

// Implement Sealed for complex types
impl private::Sealed for ComplexI16 {}
impl private::Sealed for ComplexF32 {}

// Implement Voxel for complex types
impl Voxel for ComplexI16 {
    const MIN: Self = Self { re: i16::MIN, im: i16::MIN };
    const MAX: Self = Self { re: i16::MAX, im: i16::MAX };
}

impl Voxel for ComplexF32 {
    const MIN: Self = Self { re: f32::NEG_INFINITY, im: f32::NEG_INFINITY };
    const MAX: Self = Self { re: f32::INFINITY, im: f32::INFINITY };
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
    const MIN: Self = i8::MIN;
    const MAX: Self = i8::MAX;
}

impl Voxel for i16 {
    const MIN: Self = i16::MIN;
    const MAX: Self = i16::MAX;
}

impl Voxel for u16 {
    const MIN: Self = u16::MIN;
    const MAX: Self = u16::MAX;
}

impl Voxel for f32 {
    const MIN: Self = f32::NEG_INFINITY;
    const MAX: Self = f32::INFINITY;
}

impl Voxel for half::f16 {
    const MIN: Self = half::f16::NEG_INFINITY;
    const MAX: Self = half::f16::INFINITY;
}

/// Legacy type alias for backwards compatibility
pub type Int16Complex = ComplexI16;

/// Legacy type alias for backwards compatibility  
pub type Float32Complex = ComplexF32;
