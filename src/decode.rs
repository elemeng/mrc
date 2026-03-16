//! SIMD-accelerated decoding

use crate::endian::FileEndian;
use crate::mode::{Int16Complex, Float32Complex};

use alloc::vec::Vec;

pub trait Decode: Sized {
    const BYTE_SIZE: usize;
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self;
}

impl Decode for i8 {
    const BYTE_SIZE: usize = 1;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, _endian: FileEndian) -> Self {
        bytes[offset] as i8
    }
}

impl Decode for i16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
        match endian {
            FileEndian::LittleEndian => i16::from_le_bytes(arr),
            FileEndian::BigEndian => i16::from_be_bytes(arr),
        }
    }
}

impl Decode for u16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
        match endian {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        }
    }
}

impl Decode for i32 {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 4] = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
        match endian {
            FileEndian::LittleEndian => i32::from_le_bytes(arr),
            FileEndian::BigEndian => i32::from_be_bytes(arr),
        }
    }
}

impl Decode for f32 {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 4] = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
        match endian {
            FileEndian::LittleEndian => f32::from_le_bytes(arr),
            FileEndian::BigEndian => f32::from_be_bytes(arr),
        }
    }
}

impl Decode for Int16Complex {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        Self {
            real: i16::decode(bytes, offset, endian),
            imag: i16::decode(bytes, offset + 2, endian),
        }
    }
}

impl Decode for Float32Complex {
    const BYTE_SIZE: usize = 8;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        Self {
            real: f32::decode(bytes, offset, endian),
            imag: f32::decode(bytes, offset + 4, endian),
        }
    }
}

#[cfg(feature = "f16")]
impl Decode for f16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
        let bits = match endian {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        };
        f16::from_bits(bits)
    }
}

/// Decode slice with SIMD (generic)
#[cfg(feature = "std")]
pub fn decode_slice<T: Decode>(bytes: &[u8], endian: FileEndian) -> Vec<T> {
    let n = bytes.len() / T::BYTE_SIZE;
    let mut result = Vec::with_capacity(n);

    result.reserve_exact(n);
    for i in 0..n {
        result.push(T::decode(bytes, i * T::BYTE_SIZE, endian));
    }

    result
}