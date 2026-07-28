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
impl_endian_codec!(u32, 4);
impl_endian_codec!(i64, 8);
impl_endian_codec!(f64, 8);

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

/// Decode bytes into an existing typed slice, avoiding a new allocation.
///
/// For native-endian files, this is a plain `memcpy`.  For non-native endian,
/// uses SIMD byte-swap when available.
///
/// # Errors
/// Returns `Error::TypeMismatch` if `bytes.len() != values.len() * T::BYTE_SIZE`.
///
/// # Example
/// ```rust
/// use mrc::decode_into;
/// use mrc::FileEndian;
///
/// let bytes = [0x34, 0x12, 0x78, 0x56];
/// let mut vals = [0i16, 0i16];
/// decode_into(&bytes, &mut vals, FileEndian::LittleEndian).unwrap();
/// assert_eq!(vals, [0x1234, 0x5678]);
/// ```
pub fn decode_into<T: EndianCodec + Copy>(
    bytes: &[u8],
    values: &mut [T],
    endian: FileEndian,
) -> Result<(), crate::Error> {
    let expected = values
        .len()
        .checked_mul(T::BYTE_SIZE)
        .ok_or(crate::Error::TypeMismatch {
            expected: 0,
            actual: bytes.len(),
        })?;
    if bytes.len() != expected {
        return Err(crate::Error::TypeMismatch {
            expected,
            actual: bytes.len(),
        });
    }

    // Fast path: native endian is a simple memcpy.
    if endian == FileEndian::native() {
        // SAFETY: `bytes.len() == values.len() * T::BYTE_SIZE` (checked above),
        // both slices are valid and non-overlapping.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                values.as_mut_ptr() as *mut u8,
                bytes.len(),
            );
        }
        return Ok(());
    }

    // Non-native endian: byte-swap raw bytes into the output slice.
    let dst_bytes = unsafe { core::slice::from_raw_parts_mut(values.as_mut_ptr() as *mut u8, bytes.len()) };
    swap_bytes_by_size::<T>(bytes, dst_bytes);
    Ok(())
}

/// Swap bytes between src and dst for the given element size.
/// For BYTE_SIZE=1, this is a simple memcpy (single byte = no endianness).
/// For 2/4/8-byte elements, uses SIMD when available.
fn swap_bytes_by_size<T: EndianCodec>(src: &[u8], dst: &mut [u8]) {
    match T::BYTE_SIZE {
        1 => dst.copy_from_slice(src),
        2 => {
            #[cfg(feature = "simd")]
            crate::engine::simd::swap_2byte_simd(src, dst);
            #[cfg(not(feature = "simd"))]
            for (d, s) in dst.chunks_exact_mut(2).zip(src.chunks_exact(2)) {
                d[0] = s[1]; d[1] = s[0];
            }
        }
        4 => {
            #[cfg(feature = "simd")]
            crate::engine::simd::swap_4byte_simd(src, dst);
            #[cfg(not(feature = "simd"))]
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                d[0] = s[3]; d[1] = s[2]; d[2] = s[1]; d[3] = s[0];
            }
        }
        8 => {
            #[cfg(feature = "simd")]
            crate::engine::simd::swap_8byte_simd(src, dst);
            #[cfg(not(feature = "simd"))]
            for (d, s) in dst.chunks_exact_mut(8).zip(src.chunks_exact(8)) {
                d[0] = s[7]; d[1] = s[6]; d[2] = s[5]; d[3] = s[4];
                d[4] = s[3]; d[5] = s[2]; d[6] = s[1]; d[7] = s[0];
            }
        }
        _ => unreachable!(),
    }
}

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

    // Non-native endian: byte-swap raw bytes to native order.
    {
        let dst_bytes =
            unsafe { std::slice::from_raw_parts_mut(result.as_mut_ptr() as *mut u8, bytes.len()) };

        #[cfg(feature = "parallel")]
        if n >= PAR_MIN_VOXELS {
            use rayon::prelude::*;
            let chunk_voxels = PAR_MIN_VOXELS;
            let chunk_bytes = chunk_voxels * T::BYTE_SIZE;
            dst_bytes
                .par_chunks_mut(chunk_bytes)
                .zip(bytes.par_chunks(chunk_bytes))
                .for_each(|(dst, src)| swap_bytes_by_size::<T>(src, dst));
        } else {
            swap_bytes_by_size::<T>(bytes, dst_bytes);
        }

        #[cfg(not(feature = "parallel"))]
        swap_bytes_by_size::<T>(bytes, dst_bytes);
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

    // Non-native endian: swap bytes directly from values into output buffer.
    // SAFETY: values and bytes have the same byte count (checked above) and
    // are non-overlapping (values is from the caller, bytes is a local buffer).
    let src = unsafe {
        core::slice::from_raw_parts(values.as_ptr() as *const u8, bytes.len())
    };
    swap_bytes_by_size::<T>(src, bytes);
    Ok(())
}

// ============================================================================
// Parallel minimum threshold
// ============================================================================

/// Minimum number of voxels required to trigger parallel processing.
///
/// Blocks smaller than this are processed sequentially, avoiding rayon
/// overhead for small requests.
#[cfg(feature = "parallel")]
pub(crate) const PAR_MIN_VOXELS: usize = 262_144;



// ============================================================================
// In-place byte-order swap (public API)
// ============================================================================

/// Swap byte order of typed voxel data in-place.
///
/// Converts a slice of voxels from one endianness to the other by byte-swapping
/// each element's bytes in-place.  When `from == FileEndian::native()` this is
/// a no-op — the data is already in host order.
///
/// Uses SIMD acceleration (AVX2 / NEON) when the `simd` feature is enabled
/// and the required ISA is detected at runtime.
///
/// # Example
///
/// ```rust
/// use mrc::swap_bytes_in_place;
/// use mrc::FileEndian;
///
/// let mut data = [0x1234u16.to_be(), 0x5678u16.to_be()];
/// swap_bytes_in_place(&mut data, FileEndian::BigEndian);
/// assert_eq!(data, [0x1234, 0x5678]);
/// ```
pub fn swap_bytes_in_place<T: crate::Voxel>(data: &mut [T], from: FileEndian) {
    if from == FileEndian::native() || data.is_empty() {
        return;
    }
    let byte_len = data.len() * T::BYTE_SIZE;
    // SAFETY: the byte_len calculation is exact; the pointer casts produce
    // a valid mutable byte slice of the same length.
    let bytes = unsafe { core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, byte_len) };
    // Reverse bytes within each element using in-place chunk reversal.
    // This is safe for in-place operation because `chunks_exact_mut` produces
    // non-overlapping sub-slices, and `reverse()` swaps bytes within each chunk.
    for chunk in bytes.chunks_exact_mut(T::BYTE_SIZE) {
        chunk.reverse();
    }
}

// ============================================================================
// Parallel Block Encoding
// ============================================================================

/// Encode a block with parallel processing.
///
/// Pre-allocates a single contiguous output buffer and processes it with
/// parallel chunks using `encode_slice` for SIMD-accelerated encoding.
///
/// # Errors
/// Returns `Error::TypeMismatch` if the chunk sizes don't match the buffer.
#[cfg(feature = "parallel")]
pub fn encode_block_parallel<T: EndianCodec + Sync>(
    values: &[T],
    endian: FileEndian,
) -> Result<Vec<u8>, crate::Error> {
    use rayon::prelude::*;
    let byte_len = values.len() * T::BYTE_SIZE;
    let mut buffer = vec![0u8; byte_len];

    let chunk_voxels = 262_144;
    buffer
        .par_chunks_mut(chunk_voxels * T::BYTE_SIZE)
        .zip(values.par_chunks(chunk_voxels))
        .try_for_each(|(dst, chunk)| {
            encode_slice(chunk, dst, endian)
        })?;

    Ok(buffer)
}
