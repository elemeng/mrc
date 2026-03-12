//! Encoding trait for converting between file bytes and voxel values
//!
//! This module provides a unified trait for handling endianness-aware
//! encoding/decoding of voxel data.

use crate::{FileEndian, Mode, Voxel, Error};
use crate::voxel::{ComplexI16, ComplexF32};

/// Trait for encoding/decoding voxel data with endianness handling
///
/// Each MRC mode has a corresponding encoding that knows:
/// - The mode constant
/// - How to decode bytes to voxels
/// - How to encode voxels to bytes
pub trait Encoding: Voxel {
    /// The MRC mode this encoding handles
    const MODE: Mode;
    
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
    /// 
    /// Returns error if bytes slice is too small.
    #[inline]
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::SIZE, "Buffer too small for decoding");
        // SAFETY: We just verified the size
        unsafe { Self::decode_unchecked(endian, bytes) }
    }
    
    /// Encode a single voxel to file-endian bytes (checked)
    /// 
    /// Returns error if bytes slice is too small.
    #[inline]
    fn encode(self, endian: FileEndian, bytes: &mut [u8]) {
        debug_assert!(bytes.len() >= Self::SIZE, "Buffer too small for encoding");
        // SAFETY: We just verified the size
        unsafe { self.encode_unchecked(endian, bytes) }
    }
    
    /// Decode with explicit error on bounds failure
    #[inline]
    fn decode_checked(endian: FileEndian, bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < Self::SIZE {
            return Err(Error::BufferTooSmall {
                expected: Self::SIZE,
                got: bytes.len(),
            });
        }
        // SAFETY: We just verified the size
        Ok(unsafe { Self::decode_unchecked(endian, bytes) })
    }
    
    /// Encode with explicit error on bounds failure
    #[inline]
    fn encode_checked(self, endian: FileEndian, bytes: &mut [u8]) -> Result<(), Error> {
        if bytes.len() < Self::SIZE {
            return Err(Error::BufferTooSmall {
                expected: Self::SIZE,
                got: bytes.len(),
            });
        }
        // SAFETY: We just verified the size
        unsafe { self.encode_unchecked(endian, bytes) };
        Ok(())
    }
}

// ============================================================================
// Encoding implementations
// ============================================================================

// i8 encoding (Mode 0)
impl Encoding for i8 {
    const MODE: Mode = Mode::Int8;
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

// i16 encoding (Mode 1)
impl Encoding for i16 {
    const MODE: Mode = Mode::Int16;
    const SIZE: usize = 2;
    
    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        // SAFETY: Caller ensures bytes has at least 2 elements
        let arr: [u8; 2] = unsafe { *bytes.as_ptr().cast() };
        match endian {
            FileEndian::Little => i16::from_le_bytes(arr),
            FileEndian::Big => i16::from_be_bytes(arr),
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

// f32 encoding (Mode 2)
impl Encoding for f32 {
    const MODE: Mode = Mode::Float32;
    const SIZE: usize = 4;
    
    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        // SAFETY: Caller ensures bytes has at least 4 elements
        let arr: [u8; 4] = unsafe { *bytes.as_ptr().cast() };
        match endian {
            FileEndian::Little => f32::from_le_bytes(arr),
            FileEndian::Big => f32::from_be_bytes(arr),
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

// u16 encoding (Mode 6)
impl Encoding for u16 {
    const MODE: Mode = Mode::Uint16;
    const SIZE: usize = 2;
    
    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        // SAFETY: Caller ensures bytes has at least 2 elements
        let arr: [u8; 2] = unsafe { *bytes.as_ptr().cast() };
        match endian {
            FileEndian::Little => u16::from_le_bytes(arr),
            FileEndian::Big => u16::from_be_bytes(arr),
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

// ComplexI16 encoding (Mode 3)
impl Encoding for ComplexI16 {
    const MODE: Mode = Mode::Int16Complex;
    const SIZE: usize = 4;
    
    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        // SAFETY: Caller ensures bytes has at least 4 elements
        let re = unsafe { <i16 as Encoding>::decode_unchecked(endian, &bytes[0..2]) };
        let im = unsafe { <i16 as Encoding>::decode_unchecked(endian, &bytes[2..4]) };
        Self::new(re, im)
    }
    
    #[inline]
    unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]) {
        unsafe { self.re.encode_unchecked(endian, &mut bytes[0..2]) };
        unsafe { self.im.encode_unchecked(endian, &mut bytes[2..4]) };
    }
}

// ComplexF32 encoding (Mode 4)
impl Encoding for ComplexF32 {
    const MODE: Mode = Mode::Float32Complex;
    const SIZE: usize = 8;
    
    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        // SAFETY: Caller ensures bytes has at least 8 elements
        let re = unsafe { <f32 as Encoding>::decode_unchecked(endian, &bytes[0..4]) };
        let im = unsafe { <f32 as Encoding>::decode_unchecked(endian, &bytes[4..8]) };
        Self::new(re, im)
    }
    
    #[inline]
    unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]) {
        unsafe { self.re.encode_unchecked(endian, &mut bytes[0..4]) };
        unsafe { self.im.encode_unchecked(endian, &mut bytes[4..8]) };
    }
}

// f16 encoding (Mode 12)
#[cfg(feature = "f16")]
impl Encoding for half::f16 {
    const MODE: Mode = Mode::Float16;
    const SIZE: usize = 2;
    
    #[inline]
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self {
        let arr: [u8; 2] = unsafe { *bytes.as_ptr().cast() };
        let bits = match endian {
            FileEndian::Little => u16::from_le_bytes(arr),
            FileEndian::Big => u16::from_be_bytes(arr),
        };
        half::f16::from_bits(bits)
    }
    
    #[inline]
    unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]) {
        let bits = self.to_bits();
        let arr = match endian {
            FileEndian::Little => bits.to_le_bytes(),
            FileEndian::Big => bits.to_be_bytes(),
        };
        bytes.copy_from_slice(&arr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_i8_encoding() {
        let mut buf = [0u8; 1];
        (-42i8).encode(FileEndian::Little, &mut buf);
        assert_eq!(buf[0], 214); // -42 as u8
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
    
    #[test]
    fn test_checked_encoding() {
        let buf = [0u8; 1];
        // Too small for i16
        assert!(i16::decode_checked(FileEndian::Little, &buf).is_err());
        
        let mut buf_mut = [0u8; 1];
        assert!((-1000i16).encode_checked(FileEndian::Little, &mut buf_mut).is_err());
    }
}