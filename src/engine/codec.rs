//! Bidirectional endian codec for MRC voxel types.
//!
//! The [`EndianCodec`] trait provides symmetric encode/decode operations
//! between raw bytes and typed values, handling both little-endian and
//! big-endian MRC files. Slice-level helpers (`decode_slice`, `encode_slice`)
//! use SIMD and parallel processing when features are enabled.

use super::endian::FileEndian;
use crate::mode::{Float32Complex, Int16Complex};

// ============================================================================
// EndianCodec Trait - Bidirectional endian conversion
// ============================================================================

/// Bidirectional codec for endian-normalized byte conversion.
///
/// This trait is `#[doc(hidden)]` — it is an internal plumbing trait
/// consumed by the [`Voxel`](crate::Voxel) trait.
#[doc(hidden)]
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

/// Macro to generate EndianCodec for fixed-size integer/float types.
/// All 2/4/8-byte primitives use the same pattern: read array, from_le/be_bytes.
macro_rules! impl_endian_codec {
    ($ty:ty, $size:literal) => {
        impl EndianCodec for $ty {
            const BYTE_SIZE: usize = $size;

            #[inline]
            fn from_bytes(bytes: &[u8], offset: usize, endian: FileEndian) -> Self {
                let mut arr = [0u8; $size];
                arr.copy_from_slice(&bytes[offset..offset + $size]);
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
                bytes[offset..offset + $size].copy_from_slice(&arr);
            }
        }
    };
}

impl_endian_codec!(i16, 2);
impl_endian_codec!(u16, 2);
impl_endian_codec!(i32, 4);
impl_endian_codec!(f32, 4);

impl EndianCodec for i8 {
    const BYTE_SIZE: usize = 1;

    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, _endian: FileEndian) -> Self {
        bytes[offset] as Self
    }

    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, _endian: FileEndian) {
        bytes[offset] = *self as u8;
    }
}

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
pub fn decode_slice<T: EndianCodec + Send + Copy>(
    bytes: &[u8],
    endian: FileEndian,
) -> Result<Vec<T>, crate::Error> {
    if bytes.len() % T::BYTE_SIZE != 0 {
        return Err(crate::Error::TypeMismatch {
            expected: T::BYTE_SIZE,
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

    // Non-native endian: byte-swap raw bytes to native order, then memcpy.
    #[cfg(feature = "simd")]
    {
        let dst_bytes =
            unsafe { std::slice::from_raw_parts_mut(result.as_mut_ptr() as *mut u8, bytes.len()) };
        match T::BYTE_SIZE {
            2 => crate::engine::simd::swap_2byte_simd(bytes, dst_bytes),
            4 => crate::engine::simd::swap_4byte_simd(bytes, dst_bytes),
            8 => crate::engine::simd::swap_8byte_simd(bytes, dst_bytes),
            _ => {
                per_element_decode::<T>(&mut result, bytes, n, endian);
            }
        }
    }
    #[cfg(not(feature = "simd"))]
    {
        per_element_decode::<T>(&mut result, bytes, n, endian);
    }

    // SAFETY: all n elements have been initialized above (either via SIMD swap + memcpy, or fallback).
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
pub fn encode_slice<T: EndianCodec + Sync>(
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
        // SAFETY: `bytes.len() == values.len() * T::BYTE_SIZE` (checked above),
        // both pointers are valid and non-overlapping (mutable bytes comes from
        // a separate allocation, values is an immutable reference).
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr() as *const u8,
                bytes.as_mut_ptr(),
                bytes.len(),
            );
        }
        return Ok(());
    }

    // Non-native endian: memcpy native bytes, then byte-swap in-place.
    // SAFETY: same invariants as the native path — sizes match, buffers
    // are non-overlapping; the swap functions read and write within bounds.
    unsafe {
        core::ptr::copy_nonoverlapping(
            values.as_ptr() as *const u8,
            bytes.as_mut_ptr(),
            bytes.len(),
        );
    }
    #[cfg(feature = "simd")]
    {
        match T::BYTE_SIZE {
            2 => {
                let (src, dst) = (bytes.as_ptr(), bytes.as_mut_ptr());
                let len = bytes.len();
                crate::engine::simd::swap_2byte_simd(
                    // SAFETY: `src`/`dst` point into `bytes`, which is a valid
                    // mutable slice of known length. The SIMD function respects
                    // bounds and writes every byte of the output.
                    unsafe { std::slice::from_raw_parts(src, len) },
                    unsafe { std::slice::from_raw_parts_mut(dst, len) },
                );
            }
            4 => {
                let (src, dst) = (bytes.as_ptr(), bytes.as_mut_ptr());
                let len = bytes.len();
                crate::engine::simd::swap_4byte_simd(
                    unsafe { std::slice::from_raw_parts(src, len) },
                    unsafe { std::slice::from_raw_parts_mut(dst, len) },
                );
            }
            8 => {
                let (src, dst) = (bytes.as_ptr(), bytes.as_mut_ptr());
                let len = bytes.len();
                crate::engine::simd::swap_8byte_simd(
                    unsafe { std::slice::from_raw_parts(src, len) },
                    unsafe { std::slice::from_raw_parts_mut(dst, len) },
                );
            }
            _ => {
                per_element_encode::<T>(values, bytes, endian);
            }
        }
    }
    #[cfg(not(feature = "simd"))]
    {
        per_element_encode::<T>(values, bytes, endian);
    }
    Ok(())
}

// ============================================================================
// Per-element fallback helpers (used when simd feature is disabled)
// ============================================================================

/// Per-element decode fallback for non-native endian files.
/// Used when the `simd` feature is not available.
///
/// Initializes the first `n` elements of `result` (which must have capacity ≥ n).
/// Does NOT call `set_len` — the caller is responsible for that.
fn per_element_decode<T: EndianCodec + Send>(
    result: &mut Vec<T>,
    bytes: &[u8],
    n: usize,
    endian: FileEndian,
) {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        const CHUNK_VOXELS: usize = 262_144;
        // SAFETY: `result` was allocated with `Vec::with_capacity(n)`, so
        // `as_mut_ptr()` points to at least `n` uninitialized slots and is
        // properly aligned for `T`.  Every slot is written to below (through
        // the mutable slice) before the caller calls `set_len(n)`.
        let result_slice = unsafe { std::slice::from_raw_parts_mut(result.as_mut_ptr(), n) };
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
        // SAFETY: `result` was allocated with `Vec::with_capacity(n)`, so
        // `as_mut_ptr()` points to at least `n` uninitialized slots and is
        // properly aligned for `T`.  Every slot is written to below before
        // the caller calls `set_len(n)`.
        let result_slice = unsafe { std::slice::from_raw_parts_mut(result.as_mut_ptr(), n) };
        for (i, slot) in result_slice.iter_mut().enumerate() {
            *slot = T::from_bytes(bytes, i * T::BYTE_SIZE, endian);
        }
    }
}

/// Per-element encode fallback for non-native endian files.
/// Used when the `simd` feature is not available.
fn per_element_encode<T: EndianCodec + Sync>(values: &[T], bytes: &mut [u8], endian: FileEndian) {
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
}

// ============================================================================
// Parallel Block Encoding
// ============================================================================

/// Encode a block with parallel processing.
///
/// Each chunk allocates a fresh buffer to avoid contention on thread-local
/// state and the extra clone that a shared buffer would require.  The
/// allocator reuses the same-sized memory across chunks, so the cost is
/// negligible compared to the encode work itself.
#[cfg(feature = "parallel")]
pub fn encode_block_parallel<T: EndianCodec + Sync>(
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

            let mut buffer = vec![0u8; chunk.len() * T::BYTE_SIZE];

            for (i, val) in chunk.iter().enumerate() {
                val.to_bytes(&mut buffer, i * T::BYTE_SIZE, endian);
            }

            (chunk_idx, buffer)
        })
        .collect()
}
