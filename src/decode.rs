//! SIMD-accelerated decoding

use crate::endian::FileEndian;

use alloc::vec::Vec;

/// Decode f32 from bytes with SIMD-friendly operations
#[inline]
pub fn decode_f32(bytes: &[u8], offset: usize, endian: FileEndian) -> f32 {
    let arr: [u8; 4] = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
    match endian {
        FileEndian::LittleEndian => f32::from_le_bytes(arr),
        FileEndian::BigEndian => f32::from_be_bytes(arr),
    }
}

/// Decode i16 from bytes with SIMD-friendly operations
#[inline]
pub fn decode_i16(bytes: &[u8], offset: usize, endian: FileEndian) -> i16 {
    let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
    match endian {
        FileEndian::LittleEndian => i16::from_le_bytes(arr),
        FileEndian::BigEndian => i16::from_be_bytes(arr),
    }
}

/// Decode u16 from bytes with SIMD-friendly operations
#[inline]
pub fn decode_u16(bytes: &[u8], offset: usize, endian: FileEndian) -> u16 {
    let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
    match endian {
        FileEndian::LittleEndian => u16::from_le_bytes(arr),
        FileEndian::BigEndian => u16::from_be_bytes(arr),
    }
}

/// Decode i32 from bytes
#[inline]
pub fn decode_i32(bytes: &[u8], offset: usize, endian: FileEndian) -> i32 {
    let arr: [u8; 4] = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
    match endian {
        FileEndian::LittleEndian => i32::from_le_bytes(arr),
        FileEndian::BigEndian => i32::from_be_bytes(arr),
    }
}

/// Decode f32 slice with SIMD
#[cfg(feature = "std")]
pub fn decode_f32_slice(bytes: &[u8], endian: FileEndian) -> Vec<f32> {
    let n = bytes.len() / 4;
    let mut result = Vec::with_capacity(n);

    result.reserve_exact(n);
    for i in 0..n {
        result.push(decode_f32(bytes, i * 4, endian));
    }

    result
}

/// Decode i16 slice with SIMD
#[cfg(feature = "std")]
pub fn decode_i16_slice(bytes: &[u8], endian: FileEndian) -> Vec<i16> {
    let n = bytes.len() / 2;
    let mut result = Vec::with_capacity(n);

    result.reserve_exact(n);
    for i in 0..n {
        result.push(decode_i16(bytes, i * 2, endian));
    }

    result
}