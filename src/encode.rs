//! SIMD-accelerated encoding

use crate::endian::FileEndian;
use crate::mode::{Int16Complex, Float32Complex};

use alloc::vec::Vec;

pub(crate) trait Encode: Sized {
    const BYTE_SIZE: usize;
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian);
}

impl Encode for i8 {
    const BYTE_SIZE: usize = 1;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, _endian: FileEndian) {
        bytes[offset] = *self as u8;
    }
}

impl Encode for i16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

impl Encode for u16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

impl Encode for i32 {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 4].copy_from_slice(&arr);
    }
}

impl Encode for f32 {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 4].copy_from_slice(&arr);
    }
}

impl Encode for Int16Complex {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.real.encode(bytes, offset, endian);
        self.imag.encode(bytes, offset + 2, endian);
    }
}

impl Encode for Float32Complex {
    const BYTE_SIZE: usize = 8;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.real.encode(bytes, offset, endian);
        self.imag.encode(bytes, offset + 4, endian);
    }
}

#[cfg(feature = "f16")]
impl Encode for f16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let bits = self.to_bits();
        let arr = match endian {
            FileEndian::LittleEndian => bits.to_le_bytes(),
            FileEndian::BigEndian => bits.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

/// Encode slice with SIMD (generic)
#[cfg(feature = "std")]
pub(crate) fn encode_slice<T: Encode + Sync>(values: &[T], bytes: &mut [u8], endian: FileEndian) {
    assert_eq!(values.len() * T::BYTE_SIZE, bytes.len());
    
    const CHUNK_SIZE: usize = 4096;  // Process 4KB chunks for better cache utilization
    
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        bytes
            .par_chunks_mut(CHUNK_SIZE * T::BYTE_SIZE)
            .zip(values.par_chunks(CHUNK_SIZE))
            .for_each(|(dst, src)| {
                for (i, val) in src.iter().enumerate() {
                    val.encode(dst, i * T::BYTE_SIZE, endian);
                }
            });
    }
    
    #[cfg(not(feature = "parallel"))]
    {
        for (i, val) in values.iter().enumerate() {
            val.encode(bytes, i * T::BYTE_SIZE, endian);
        }
    }
}

/// Thread-local encoding buffer for parallel file writing
#[cfg(all(feature = "std", feature = "parallel"))]
use std::thread_local;

#[cfg(all(feature = "std", feature = "parallel"))]
thread_local! {
    static ENCODE_BUFFER: std::cell::RefCell<Vec<u8>> = 
        std::cell::RefCell::new(Vec::with_capacity(4 * 1024 * 1024)); // 4MB buffer
}

/// Encode a block with parallel processing and thread-local buffers
/// Returns Vec<Vec<u8>> containing encoded chunks for parallel writing
#[cfg(all(feature = "std", feature = "parallel"))]
pub(crate) fn encode_block_parallel<T: Encode + Sync + Clone>(
    values: &[T],
    chunk_size: usize,
    endian: FileEndian,
) -> Vec<(usize, Vec<u8>)> {
    use rayon::prelude::*;
    
    let chunk_count = (values.len() + chunk_size - 1) / chunk_size;
    
    (0..chunk_count)
        .into_par_iter()
        .map(|chunk_idx| {
            let start = chunk_idx * chunk_size;
            let end = (start + chunk_size).min(values.len());
            let chunk = &values[start..end];
            
            ENCODE_BUFFER.with(|buf| {
                let mut buffer = buf.borrow_mut();
                buffer.clear();
                buffer.resize(chunk.len() * T::BYTE_SIZE, 0);
                
                for (i, val) in chunk.iter().enumerate() {
                    val.encode(&mut buffer, i * T::BYTE_SIZE, endian);
                }
                
                (chunk_idx, buffer.clone())
            })
        })
        .collect()
}

/// SIMD-accelerated encode for i16 with endian conversion
#[cfg(feature = "std")]
pub(crate) fn encode_i16_slice_simd(values: &[i16], bytes: &mut [u8], endian: FileEndian) {
    use crate::simd::swap_endian_i16;
    
    assert_eq!(values.len() * 2, bytes.len());
    
    // Copy values as bytes
    unsafe {
        let src_ptr = values.as_ptr() as *const u8;
        let dst_ptr = bytes.as_mut_ptr();
        core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, bytes.len());
    }
    
    // Apply endian conversion with SIMD
    if endian == FileEndian::BigEndian && cfg!(target_endian = "little") {
        let mut values_mut: Vec<i16> = values.to_vec();
        swap_endian_i16(&mut values_mut);
        
        unsafe {
            let src_ptr = values_mut.as_ptr() as *const u8;
            let dst_ptr = bytes.as_mut_ptr();
            core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, bytes.len());
        }
    }
}

/// SIMD-accelerated encode for f32 with endian conversion
#[cfg(feature = "std")]
pub(crate) fn encode_f32_slice_simd(values: &[f32], bytes: &mut [u8], endian: FileEndian) {
    use crate::simd::swap_endian_f32;
    
    assert_eq!(values.len() * 4, bytes.len());
    
    // Copy values as bytes
    unsafe {
        let src_ptr = values.as_ptr() as *const u8;
        let dst_ptr = bytes.as_mut_ptr();
        core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, bytes.len());
    }
    
    // Apply endian conversion with SIMD
    if endian == FileEndian::BigEndian && cfg!(target_endian = "little") {
        let mut values_mut: Vec<f32> = values.to_vec();
        swap_endian_f32(&mut values_mut);
        
        unsafe {
            let src_ptr = values_mut.as_ptr() as *const u8;
            let dst_ptr = bytes.as_mut_ptr();
            core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, bytes.len());
        }
    }
}

/// SIMD-accelerated encode for f32 to i16 conversion
#[cfg(feature = "std")]
pub(crate) fn encode_f32_to_i16_simd(values: &[f32], bytes: &mut [u8], endian: FileEndian) {
    use crate::simd::convert_f32_to_i16;
    
    assert_eq!(values.len() * 2, bytes.len());
    
    // Convert f32 to i16 with SIMD
    let mut i16_data = Vec::with_capacity(values.len());
    i16_data.resize(values.len(), 0);
    convert_f32_to_i16(values, &mut i16_data);
    
    // Encode i16 with SIMD
    encode_i16_slice_simd(&i16_data, bytes, endian);
}