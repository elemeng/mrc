//! Type Conversion Layer (Layer 4 of the pipeline)
//!
//! This module implements voxel type conversions:
//! ```text
//! Typed voxel values → Converted voxel values
//! ```
//!
//! Conversions include:
//! - Int8 → Float32
//! - Int16 → Float32
//! - Uint16 → Float32
//! - Float16 → Float32
//! - Float32 → Int16
//! - Int16Complex ↔ Float32Complex

use crate::mode::{Float32Complex, Int16Complex};

/// Trait for converting between voxel types.
///
/// This enables the type-level conversion graph described in engine.md.
pub trait Convert<S>: Sized {
    /// Convert a source value to the destination type
    fn convert(src: S) -> Self;
}

// === Conversions TO Float32 ===

impl Convert<i8> for f32 {
    #[inline]
    fn convert(src: i8) -> Self {
        src as f32
    }
}

impl Convert<i16> for f32 {
    #[inline]
    fn convert(src: i16) -> Self {
        src as f32
    }
}

impl Convert<u16> for f32 {
    #[inline]
    fn convert(src: u16) -> Self {
        src as f32
    }
}

#[cfg(feature = "f16")]
impl Convert<f16> for f32 {
    #[inline]
    fn convert(src: f16) -> Self {
        src as f32
    }
}

impl Convert<f32> for f32 {
    #[inline]
    fn convert(src: f32) -> Self {
        src
    }
}

// === Conversions FROM Float32 ===

impl Convert<f32> for i8 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(i8::MIN as f32, i8::MAX as f32) as i8
    }
}

impl Convert<f32> for i16 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(i16::MIN as f32, i16::MAX as f32) as i16
    }
}

impl Convert<f32> for u16 {
    #[inline]
    fn convert(src: f32) -> Self {
        src.clamp(u16::MIN as f32, u16::MAX as f32) as u16
    }
}

#[cfg(feature = "f16")]
impl Convert<f32> for f16 {
    #[inline]
    fn convert(src: f32) -> Self {
        src as f16
    }
}

// === Identity conversions ===

impl Convert<i8> for i8 {
    #[inline]
    fn convert(src: i8) -> Self {
        src
    }
}

impl Convert<i16> for i16 {
    #[inline]
    fn convert(src: i16) -> Self {
        src
    }
}

impl Convert<u16> for u16 {
    #[inline]
    fn convert(src: u16) -> Self {
        src
    }
}

// === Complex type conversions ===

impl Convert<Int16Complex> for Float32Complex {
    #[inline]
    fn convert(src: Int16Complex) -> Self {
        Self {
            real: src.real as f32,
            imag: src.imag as f32,
        }
    }
}

impl Convert<Float32Complex> for Int16Complex {
    #[inline]
    fn convert(src: Float32Complex) -> Self {
        Self {
            real: src.real.clamp(i16::MIN as f32, i16::MAX as f32) as i16,
            imag: src.imag.clamp(i16::MIN as f32, i16::MAX as f32) as i16,
        }
    }
}

impl Convert<Int16Complex> for Int16Complex {
    #[inline]
    fn convert(src: Int16Complex) -> Self {
        src
    }
}

impl Convert<Float32Complex> for Float32Complex {
    #[inline]
    fn convert(src: Float32Complex) -> Self {
        src
    }
}
