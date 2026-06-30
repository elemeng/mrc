//! Bidirectional endian codec for MRC voxel types.
//!
//! The `EndianCodec` trait provides symmetric encode/decode operations
//! between raw bytes and typed values, handling both little-endian and
//! big-endian MRC files.

use super::endian::FileEndian;
use crate::mode::{Float32Complex, Int16Complex};

use std::vec::Vec;

// ============================================================================
// EndianCodec Trait - Bidirectional endian conversion
// ============================================================================

/// Bidirectional codec for endian-normalized byte conversion.
///
/// Provides symmetric encode/decode operations with guaranteed consistency.
///
/// # Example
/// ```ignore
/// // EndianCodec is an internal trait; this example is for crate developers.
/// use mrc::engine::codec::EndianCodec;
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
        let arr: [u8; 4] = [
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ];
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
        let arr: [u8; 4] = [
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ];
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
impl EndianCodec for crate::f16 {
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
/// Uses 1MB chunks for optimal cache behaviour when the `parallel` feature is enabled.
/// For native-endian files, this is a plain `memcpy`.
///
/// # Errors
/// Returns `Error::TypeMismatch` if `bytes.len()` is not a multiple of `T::BYTE_SIZE`.
pub(crate) fn decode_slice<T: EndianCodec + Send + Copy>(
    bytes: &[u8],
    endian: FileEndian,
) -> Result<Vec<T>, crate::Error> {
    if bytes.len() % T::BYTE_SIZE != 0 {
        return Err(crate::Error::TypeMismatch {
            expected: 0,
            actual: bytes.len(),
        });
    }
    let n = bytes.len() / T::BYTE_SIZE;
    let mut result = Vec::with_capacity(n);

    // Fast path: native endian is a simple memcpy.
    if endian == FileEndian::native() {
        // SAFETY: result has capacity n; we copy exactly n * BYTE_SIZE bytes,
        // fully initializing every element before setting the length.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                result.as_mut_ptr() as *mut u8,
                bytes.len(),
            );
            result.set_len(n);
        }
        return Ok(result);
    }

    // SAFETY: result has capacity n; every element is initialized below.
    let result_slice = unsafe { std::slice::from_raw_parts_mut(result.as_mut_ptr(), n) };

    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        const CHUNK_VOXELS: usize = 262_144;
        result_slice
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
        for (i, slot) in result_slice.iter_mut().enumerate() {
            *slot = T::from_bytes(bytes, i * T::BYTE_SIZE, endian);
        }
    }

    // SAFETY: all n elements have been initialized above.
    unsafe {
        result.set_len(n);
    }
    Ok(result)
}

// ============================================================================
// Slice Operations - Encode
// ============================================================================

/// Encode a slice of values to bytes with automatic parallel processing.
///
/// Uses 1MB chunks for optimal cache behaviour when the `parallel` feature is enabled.
/// For native-endian files, this is a plain `memcpy`.
///
/// # Errors
/// Returns `Error::TypeMismatch` if `bytes.len()` does not match `values.len() * T::BYTE_SIZE`.
pub(crate) fn encode_slice<T: EndianCodec + Sync>(
    values: &[T],
    bytes: &mut [u8],
    endian: FileEndian,
) -> Result<(), crate::Error> {
    if values.len().checked_mul(T::BYTE_SIZE) != Some(bytes.len()) {
        return Err(crate::Error::TypeMismatch {
            expected: values.len() * T::BYTE_SIZE,
            actual: bytes.len(),
        });
    }

    // Fast path: native endian is a simple memcpy.
    if endian == FileEndian::native() {
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr() as *const u8,
                bytes.as_mut_ptr(),
                bytes.len(),
            );
        }
        return Ok(());
    }

    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        const CHUNK_VOXELS: usize = 262_144;
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
    Ok(())
}

// ============================================================================
// Parallel Block Encoding
// ============================================================================

#[cfg(feature = "parallel")]
use std::thread_local;

#[cfg(feature = "parallel")]
thread_local! {
    static ENCODE_BUFFER: std::cell::RefCell<Vec<u8>> =
        std::cell::RefCell::new(Vec::with_capacity(4 * 1024 * 1024));
}

/// Encode a block with parallel processing and thread-local buffers.
#[cfg(feature = "parallel")]
pub(crate) fn encode_block_parallel<T: EndianCodec + Sync>(
    values: &[T],
    chunk_size: usize,
    endian: FileEndian,
) -> Vec<(usize, Vec<u8>)> {
    use rayon::prelude::*;

    let chunk_count = values.len().div_ceil(chunk_size);

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
