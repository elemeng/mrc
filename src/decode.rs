//! SIMD-accelerated decoding

use crate::endian::FileEndian;
use crate::mode::{Int16Complex, Float32Complex};

use alloc::vec::Vec;

pub(crate) trait Decode: Sized {
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
pub(crate) fn decode_slice<T: Decode + Send + Copy>(bytes: &[u8], endian: FileEndian) -> Vec<T> {
    let n = bytes.len() / T::BYTE_SIZE;
    let mut result = Vec::with_capacity(n);
    result.resize(n, unsafe { core::mem::zeroed() });
    
    const CHUNK_SIZE: usize = 4096;  // Process 4KB chunks for better cache utilization
    
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        result
            .par_chunks_mut(CHUNK_SIZE)
            .zip(bytes.par_chunks(CHUNK_SIZE * T::BYTE_SIZE))
            .for_each(|(dst, src)| {
                for (i, val) in dst.iter_mut().enumerate() {
                    *val = T::decode(src, i * T::BYTE_SIZE, endian);
                }
            });
    }
    
    #[cfg(not(feature = "parallel"))]
    {
        for i in 0..n {
            result[i] = T::decode(bytes, i * T::BYTE_SIZE, endian);
        }
    }
    
    result
}

/// SIMD-accelerated decode for i16 with endian conversion
#[cfg(feature = "std")]
pub(crate) fn decode_i16_slice_simd(bytes: &[u8], endian: FileEndian) -> Vec<i16> {
    use crate::simd::swap_endian_i16;
    
    let n = bytes.len() / 2;
    let mut result: Vec<i16> = Vec::with_capacity(n);
    result.resize(n, 0);
    
    unsafe {
        let ptr = result.as_mut_ptr() as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    }
    
    // Apply endian conversion with SIMD
    if endian == FileEndian::BigEndian && cfg!(target_endian = "little") {
        swap_endian_i16(&mut result);
    }
    
    result
}

/// SIMD-accelerated decode for f32 with endian conversion
#[cfg(feature = "std")]
pub(crate) fn decode_f32_slice_simd(bytes: &[u8], endian: FileEndian) -> Vec<f32> {
    use crate::simd::swap_endian_f32;
    
    let n = bytes.len() / 4;
    let mut result: Vec<f32> = Vec::with_capacity(n);
    result.resize(n, 0.0);
    
    unsafe {
        let ptr = result.as_mut_ptr() as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    }
    
    // Apply endian conversion with SIMD
    if endian == FileEndian::BigEndian && cfg!(target_endian = "little") {
        swap_endian_f32(&mut result);
    }
    
    result
}

/// SIMD-accelerated decode for i16 to f32 conversion
#[cfg(feature = "std")]
pub(crate) fn decode_i16_to_f32_simd(bytes: &[u8], endian: FileEndian) -> Vec<f32> {
    use crate::simd::convert_i16_to_f32;
    
    // First decode as i16
    let i16_data = decode_i16_slice_simd(bytes, endian);
    
    // Convert to f32 with SIMD
    let mut f32_data = Vec::with_capacity(i16_data.len());
    f32_data.resize(i16_data.len(), 0.0);
    convert_i16_to_f32(&i16_data, &mut f32_data);
    
    f32_data
}