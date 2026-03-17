//! Bidirectional endian codec for Layer 2 of the pipeline
//!
//! This module implements the endian normalization layer:
//! ```text
//! Raw Bytes ↔ Typed Values
//! ```
//!
//! The `EndianCodec` trait provides symmetric encode/decode operations,
//! guaranteeing that encoding and decoding are always consistent.

use super::endian::FileEndian;
use crate::mode::{Int16Complex, Float32Complex};

use alloc::vec::Vec;

// ============================================================================
// EndianCodec Trait - Bidirectional endian conversion
// ============================================================================

/// Bidirectional codec for endian-normalized byte conversion.
///
/// This is the core abstraction for Layer 2 of the pipeline.
/// Provides symmetric encode/decode operations with guaranteed consistency.
///
/// # Example
/// ```
/// use mrc::EndianCodec;
/// use mrc::FileEndian;
///
/// let value: i16 = 0x1234;
/// let mut bytes = [0u8; 2];
/// value.encode(&mut bytes, 0, FileEndian::LittleEndian);
/// let decoded = i16::decode(&bytes, 0, FileEndian::LittleEndian);
/// assert_eq!(value, decoded);
/// ```
pub trait EndianCodec: Sized {
    /// Size in bytes for one value of this type
    const BYTE_SIZE: usize;
    
    /// Decode: bytes → value (read from bytes at offset)
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self;
    
    /// Encode: value → bytes (write to bytes at offset)
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian);
    
    /// Decode alias: bytes → value
    #[inline]
    fn decode(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        Self::from_bytes(bytes, offset, endian)
    }
    
    /// Encode alias: value → bytes
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.to_bytes(bytes, offset, endian)
    }
}

// ============================================================================
// Primitive Implementations
// ============================================================================

impl EndianCodec for i8 {
    const BYTE_SIZE: usize = 1;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, _endian: FileEndian) -> Self {
        bytes[offset] as i8
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, _endian: FileEndian) {
        bytes[offset] = *self as u8;
    }
}

impl EndianCodec for i16 {
    const BYTE_SIZE: usize = 2;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
        match endian {
            FileEndian::LittleEndian => Self::from_le_bytes(arr),
            FileEndian::BigEndian => Self::from_be_bytes(arr),
        }
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

impl EndianCodec for u16 {
    const BYTE_SIZE: usize = 2;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
        match endian {
            FileEndian::LittleEndian => Self::from_le_bytes(arr),
            FileEndian::BigEndian => Self::from_be_bytes(arr),
        }
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

impl EndianCodec for i32 {
    const BYTE_SIZE: usize = 4;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 4] = [bytes[offset], bytes[offset + 1], 
                           bytes[offset + 2], bytes[offset + 3]];
        match endian {
            FileEndian::LittleEndian => Self::from_le_bytes(arr),
            FileEndian::BigEndian => Self::from_be_bytes(arr),
        }
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 4].copy_from_slice(&arr);
    }
}

impl EndianCodec for f32 {
    const BYTE_SIZE: usize = 4;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 4] = [bytes[offset], bytes[offset + 1],
                           bytes[offset + 2], bytes[offset + 3]];
        match endian {
            FileEndian::LittleEndian => Self::from_le_bytes(arr),
            FileEndian::BigEndian => Self::from_be_bytes(arr),
        }
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 4].copy_from_slice(&arr);
    }
}

// ============================================================================
// Complex Type Implementations
// ============================================================================

impl EndianCodec for Int16Complex {
    const BYTE_SIZE: usize = 4;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        Self {
            real: i16::from_bytes(bytes, offset, endian),
            imag: i16::from_bytes(bytes, offset + 2, endian),
        }
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.real.to_bytes(bytes, offset, endian);
        self.imag.to_bytes(bytes, offset + 2, endian);
    }
}

impl EndianCodec for Float32Complex {
    const BYTE_SIZE: usize = 8;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        Self {
            real: f32::from_bytes(bytes, offset, endian),
            imag: f32::from_bytes(bytes, offset + 4, endian),
        }
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.real.to_bytes(bytes, offset, endian);
        self.imag.to_bytes(bytes, offset + 4, endian);
    }
}

#[cfg(feature = "f16")]
impl EndianCodec for f16 {
    const BYTE_SIZE: usize = 2;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
        let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
        let bits = match endian {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        };
        Self::from_bits(bits)
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let bits = self.to_bits();
        let arr = match endian {
            FileEndian::LittleEndian => bits.to_le_bytes(),
            FileEndian::BigEndian => bits.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

// ============================================================================
// Slice Operations - Decode
// ============================================================================

/// Decode a slice of values from bytes with automatic parallel processing.
///
/// This is the primary entry point for Layer 2 decoding.
/// Uses 1MB chunks for optimal cache behavior.
#[cfg(feature = "std")]
pub fn decode_slice<T: EndianCodec + Send + Copy + Default>(
    bytes: &[u8],
    endian: FileEndian
) -> Vec<T> {
    let n = bytes.len() / T::BYTE_SIZE;
    let mut result = Vec::with_capacity(n);
    result.resize(n, T::default());
    
    // 1MB chunks for cache efficiency
    const CHUNK_VOXELS: usize = 262_144;
    
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        result
            .par_chunks_mut(CHUNK_VOXELS)
            .zip(bytes.par_chunks(CHUNK_VOXELS * T::BYTE_SIZE))
            .for_each(|(dst, src)| {
                for (i, val) in dst.iter_mut().enumerate() {
                    *val = T::from_bytes(src, i * T::BYTE_SIZE, endian);
                }
            });
    }
    
    #[cfg(not(feature = "parallel"))]
    {
        for i in 0..n {
            result[i] = T::from_bytes(bytes, i * T::BYTE_SIZE, endian);
        }
    }
    
    result
}

// ============================================================================
// Slice Operations - Encode
// ============================================================================

/// Encode a slice of values to bytes with automatic parallel processing.
///
/// This is the primary entry point for Layer 2 encoding.
/// Uses 1MB chunks for optimal cache behavior.
#[cfg(feature = "std")]
pub fn encode_slice<T: EndianCodec + Sync>(
    values: &[T],
    bytes: &mut [u8],
    endian: FileEndian
) {
    assert_eq!(values.len() * T::BYTE_SIZE, bytes.len());
    
    // 1MB chunks for cache efficiency
    const CHUNK_VOXELS: usize = 262_144;
    
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        bytes
            .par_chunks_mut(CHUNK_VOXELS * T::BYTE_SIZE)
            .zip(values.par_chunks(CHUNK_VOXELS))
            .for_each(|(dst, src)| {
                for (i, val) in src.iter().enumerate() {
                    val.to_bytes(dst, i * T::BYTE_SIZE, endian);
                }
            });
    }
    
    #[cfg(not(feature = "parallel"))]
    {
        for (i, val) in values.iter().enumerate() {
            val.to_bytes(bytes, i * T::BYTE_SIZE, endian);
        }
    }
}

// ============================================================================
// Parallel Block Encoding
// ============================================================================

#[cfg(all(feature = "std", feature = "parallel"))]
use std::thread_local;

#[cfg(all(feature = "std", feature = "parallel"))]
thread_local! {
    static ENCODE_BUFFER: std::cell::RefCell<Vec<u8>> = 
        std::cell::RefCell::new(Vec::with_capacity(4 * 1024 * 1024));
}

/// Encode a block with parallel processing and thread-local buffers.
#[cfg(all(feature = "std", feature = "parallel"))]
pub fn encode_block_parallel<T: EndianCodec + Sync + Clone>(
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
                    val.to_bytes(&mut buffer, i * T::BYTE_SIZE, endian);
                }
                
                (chunk_idx, buffer.clone())
            })
        })
        .collect()
}
