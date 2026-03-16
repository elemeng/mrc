//! SIMD-accelerated encoding

use crate::endian::FileEndian;

/// Encode f32 to bytes with SIMD-friendly operations
#[inline]
pub fn encode_f32(value: f32, bytes: &mut [u8], offset: usize, endian: FileEndian) {
    let arr = match endian {
        FileEndian::LittleEndian => value.to_le_bytes(),
        FileEndian::BigEndian => value.to_be_bytes(),
    };
    bytes[offset..offset + 4].copy_from_slice(&arr);
}

/// Encode i16 to bytes with SIMD-friendly operations
#[inline]
pub fn encode_i16(value: i16, bytes: &mut [u8], offset: usize, endian: FileEndian) {
    let arr = match endian {
        FileEndian::LittleEndian => value.to_le_bytes(),
        FileEndian::BigEndian => value.to_be_bytes(),
    };
    bytes[offset..offset + 2].copy_from_slice(&arr);
}

/// Encode i32 to bytes
#[inline]
pub fn encode_i32(value: i32, bytes: &mut [u8], offset: usize, endian: FileEndian) {
    let arr = match endian {
        FileEndian::LittleEndian => value.to_le_bytes(),
        FileEndian::BigEndian => value.to_be_bytes(),
    };
    bytes[offset..offset + 4].copy_from_slice(&arr);
}

/// Encode f32 slice with SIMD
#[cfg(feature = "std")]
pub fn encode_f32_slice(values: &[f32], bytes: &mut [u8], endian: FileEndian) {
    assert_eq!(values.len() * 4, bytes.len());

    for (i, &val) in values.iter().enumerate() {
        encode_f32(val, bytes, i * 4, endian);
    }
}

/// Encode i16 slice with SIMD
#[cfg(feature = "std")]
pub fn encode_i16_slice(values: &[i16], bytes: &mut [u8], endian: FileEndian) {
    assert_eq!(values.len() * 2, bytes.len());

    for (i, &val) in values.iter().enumerate() {
        encode_i16(val, bytes, i * 2, endian);
    }
}