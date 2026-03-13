//! Encoding/Decoding trait for converting between file bytes and voxel values
//!
//! This module provides a unified trait for handling endianness-aware
//! encoding/decoding of voxel data.

use crate::voxel::{ComplexF32, ComplexI16, FileEndian, Packed4Bit, Voxel};

/// Trait for encoding/decoding voxel data with endianness handling
///
/// This trait is sealed via the `Voxel` supertrait and cannot be implemented
/// outside this crate. It is used internally by `Volume` for type-safe byte
/// encoding/decoding. Users typically don't interact with this trait directly.
///
/// Inherits MODE from Voxel trait. Each implementation knows:
/// - How to decode bytes to voxels
/// - How to encode voxels to bytes
pub trait Encoding: Voxel {
    /// Size of one voxel in bytes
    const SIZE: usize;

    /// Decode a single voxel from file-endian bytes
    ///
    /// # Safety
    /// `bytes` must have at least SIZE elements.
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self;

    /// Encode a single voxel to file-endian bytes
    ///
    /// # Safety
    /// `bytes` must have at least SIZE elements.
    unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]);

    /// Decode a single voxel from file-endian bytes (checked)
    #[inline]
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::SIZE, "Buffer too small for decoding");
        // SAFETY: We just verified the size
        unsafe { Self::decode_unchecked(endian, bytes) }
    }

    /// Encode a single voxel to file-endian bytes (checked)
    #[inline]
    fn encode(self, endian: FileEndian, bytes: &mut [u8]) {
        debug_assert!(bytes.len() >= Self::SIZE, "Buffer too small for encoding");
        // SAFETY: We just verified the size
        unsafe { self.encode_unchecked(endian, bytes) }
    }
}

// ============================================================================
// Encoding implementations
// ============================================================================

// Macro for primitive types with from/to_bytes methods
macro_rules! impl_primitive_encoding {
    ($type:ty, $size:expr) => {
        impl Encoding for $type {
            const SIZE: usize = $size;

            #[inline]
            unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
                let arr: [u8; $size] = unsafe { *bytes.as_ptr().cast() };
                match endian {
                    FileEndian::Little => <$type>::from_le_bytes(arr),
                    FileEndian::Big => <$type>::from_be_bytes(arr),
                }
            }

            #[inline]
            unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]) {
                let arr = match endian {
                    FileEndian::Little => self.to_le_bytes(),
                    FileEndian::Big => self.to_be_bytes(),
                };
                bytes.copy_from_slice(&arr);
            }
        }
    };
}

// i8 encoding (Mode 0) - no endianness handling needed
impl Encoding for i8 {
    const SIZE: usize = 1;

    #[inline]
    unsafe fn decode_unchecked(_endian: FileEndian, bytes: &[u8]) -> Self {
        bytes[0] as i8
    }

    #[inline]
    unsafe fn encode_unchecked(self, _endian: FileEndian, bytes: &mut [u8]) {
        bytes[0] = self as u8;
    }
}

// Primitive type encodings
impl_primitive_encoding!(i16, 2);
impl_primitive_encoding!(u16, 2);
impl_primitive_encoding!(f32, 4);

// Macro for complex type encodings
macro_rules! impl_complex_encoding {
    ($type:ty, $real:ty, $size:expr) => {
        impl Encoding for $type {
            const SIZE: usize = $size;

            #[inline]
            unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
                let re = unsafe {
                    <$real as Encoding>::decode_unchecked(endian, &bytes[0..($size / 2)])
                };
                let im = unsafe {
                    <$real as Encoding>::decode_unchecked(endian, &bytes[($size / 2)..$size])
                };
                Self::new(re, im)
            }

            #[inline]
            unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]) {
                unsafe { self.re.encode_unchecked(endian, &mut bytes[0..($size / 2)]) };
                unsafe {
                    self.im
                        .encode_unchecked(endian, &mut bytes[($size / 2)..$size])
                };
            }
        }
    };
}

// Complex type encodings
impl_complex_encoding!(ComplexI16, i16, 4);
impl_complex_encoding!(ComplexF32, f32, 8);

// f16 encoding (Mode 12)
#[cfg(feature = "f16")]
impl Encoding for half::f16 {
    const SIZE: usize = 2;

    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        let bits = unsafe { <u16 as Encoding>::decode_unchecked(endian, bytes) };
        half::f16::from_bits(bits)
    }

    #[inline]
    unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]) {
        unsafe { self.to_bits().encode_unchecked(endian, bytes) };
    }
}

// Packed4Bit encoding (Mode 101)
impl Encoding for Packed4Bit {
    const SIZE: usize = 1;

    #[inline]
    unsafe fn decode_unchecked(_endian: FileEndian, bytes: &[u8]) -> Self {
        Self::new(bytes[0])
    }

    #[inline]
    unsafe fn encode_unchecked(self, _endian: FileEndian, bytes: &mut [u8]) {
        bytes[0] = self.byte();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i8_encoding() {
        let mut buf = [0u8; 1];
        (-42i8).encode(FileEndian::Little, &mut buf);
        assert_eq!(buf[0], 214);
        assert_eq!(i8::decode(FileEndian::Little, &buf), -42);
    }

    #[test]
    fn test_i16_encoding() {
        let mut buf = [0u8; 2];
        (-1000i16).encode(FileEndian::Little, &mut buf);
        assert_eq!(i16::decode(FileEndian::Little, &buf), -1000);

        (-1000i16).encode(FileEndian::Big, &mut buf);
        assert_eq!(i16::decode(FileEndian::Big, &buf), -1000);
    }

    #[test]
    fn test_f32_encoding() {
        let mut buf = [0u8; 4];
        3.14159f32.encode(FileEndian::Little, &mut buf);
        let result = f32::decode(FileEndian::Little, &buf);
        assert!((result - 3.14159).abs() < 0.00001);
    }
}
