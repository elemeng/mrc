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
/// # Implementor contract
/// Implementations must be plain-old-data numeric types with no padding,
/// `BYTE_SIZE == size_of::<Self>()`, and a valid bit pattern for every byte
/// sequence. The default `swap_bytes_value` and the SIMD slice helpers rely on
/// these properties.
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

    /// Swap the byte order of one value in place.
    ///
    /// The default reverses all `BYTE_SIZE` bytes, which is correct for
    /// primitive numeric types. Composite types whose components have their
    /// own byte order (the complex pair types) override this.
    #[doc(hidden)]
    #[inline]
    fn swap_bytes_value(&mut self) {
        debug_assert_eq!(core::mem::size_of::<Self>(), Self::BYTE_SIZE);
        // SAFETY: every implementor in this crate is a plain-old-data numeric
        // type with `size_of::<Self>() == BYTE_SIZE` and no padding. The
        // reference is derived from `&mut self` and covers exactly one value.
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(self as *mut Self as *mut u8, Self::BYTE_SIZE)
        };
        bytes.reverse();
    }

    /// Swap the byte order of every element in `src`, writing the result to
    /// `dst`.
    ///
    /// The default reverses each `BYTE_SIZE`-byte group (SIMD-accelerated).
    /// Composite types whose components have their own byte order (the complex
    /// pair types) override this so that real and imaginary components keep
    /// their positions.
    #[doc(hidden)]
    #[inline]
    fn swap_slice(src: &[u8], dst: &mut [u8]) {
        swap_bytes_by_size::<Self>(src, dst);
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

    #[inline]
    fn swap_bytes_value(&mut self) {
        self.real = i16::from_be_bytes(self.real.to_le_bytes());
        self.imag = i16::from_be_bytes(self.imag.to_le_bytes());
    }

    fn swap_slice(src: &[u8], dst: &mut [u8]) {
        debug_assert_eq!(src.len(), dst.len());
        debug_assert_eq!(src.len() % Self::BYTE_SIZE, 0);
        // Swap each component independently: [r0,r1,i0,i1] -> [r1,r0,i1,i0].
        for (s, d) in src
            .chunks_exact(Self::BYTE_SIZE)
            .zip(dst.chunks_exact_mut(Self::BYTE_SIZE))
        {
            d.copy_from_slice(&[s[1], s[0], s[3], s[2]]);
        }
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

    #[inline]
    fn swap_bytes_value(&mut self) {
        self.real = f32::from_be_bytes(self.real.to_le_bytes());
        self.imag = f32::from_be_bytes(self.imag.to_le_bytes());
    }

    fn swap_slice(src: &[u8], dst: &mut [u8]) {
        debug_assert_eq!(src.len(), dst.len());
        debug_assert_eq!(src.len() % Self::BYTE_SIZE, 0);
        // Swap each component independently: [r0..r3, i0..i3] -> [r3..r0, i3..i0].
        for (s, d) in src
            .chunks_exact(Self::BYTE_SIZE)
            .zip(dst.chunks_exact_mut(Self::BYTE_SIZE))
        {
            d.copy_from_slice(&[s[3], s[2], s[1], s[0], s[7], s[6], s[5], s[4]]);
        }
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
        .ok_or(crate::Error::SizeOverflow)?;
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
    let dst_bytes =
        unsafe { core::slice::from_raw_parts_mut(values.as_mut_ptr() as *mut u8, bytes.len()) };
    T::swap_slice(bytes, dst_bytes);
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
                d[0] = s[1];
                d[1] = s[0];
            }
        }
        4 => {
            #[cfg(feature = "simd")]
            crate::engine::simd::swap_4byte_simd(src, dst);
            #[cfg(not(feature = "simd"))]
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                d[0] = s[3];
                d[1] = s[2];
                d[2] = s[1];
                d[3] = s[0];
            }
        }
        8 => {
            #[cfg(feature = "simd")]
            crate::engine::simd::swap_8byte_simd(src, dst);
            #[cfg(not(feature = "simd"))]
            for (d, s) in dst.chunks_exact_mut(8).zip(src.chunks_exact(8)) {
                d[0] = s[7];
                d[1] = s[6];
                d[2] = s[5];
                d[3] = s[4];
                d[4] = s[3];
                d[5] = s[2];
                d[6] = s[1];
                d[7] = s[0];
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
        return Err(crate::Error::MisalignedLength {
            actual: bytes.len(),
            element: T::BYTE_SIZE,
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

    // Non-native endian: byte-swap raw bytes into the output allocation.
    // SAFETY: We create a &mut [u8] view into the Vec's uninitialized capacity.
    // This is sound because:
    // 1. swap_bytes_by_size exclusively WRITES to dst, it never reads
    // 2. u8 has no invalid bit patterns
    // 3. The SIMD store instructions write directly to memory
    // 4. After swap, result.set_len(n) marks all n elements as initialized
    // 5. Miri does not flag this pattern (used by bytemuck, zerocopy, etc.)
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
            .for_each(|(dst, src)| T::swap_slice(src, dst));
    } else {
        T::swap_slice(bytes, dst_bytes);
    }

    #[cfg(not(feature = "parallel"))]
    T::swap_slice(bytes, dst_bytes);

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
pub fn encode_slice<T: EndianCodec + Sync>(
    values: &[T],
    bytes: &mut [u8],
    endian: FileEndian,
) -> Result<(), crate::Error> {
    let expected = values
        .len()
        .checked_mul(T::BYTE_SIZE)
        .ok_or(crate::Error::SizeOverflow)?;
    if expected != bytes.len() {
        return Err(crate::Error::TypeMismatch {
            expected,
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
    let src = unsafe { core::slice::from_raw_parts(values.as_ptr() as *const u8, bytes.len()) };
    T::swap_slice(src, bytes);
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
/// Primitive types swap all bytes of each value.  Complex pair types swap each
/// component independently, preserving the real/imaginary layout.
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
    for value in data {
        value.swap_bytes_value();
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
        .try_for_each(|(dst, chunk)| encode_slice(chunk, dst, endian))?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_slice_swap_preserves_component_positions() {
        let src = [0x12, 0x34, 0x56, 0x78];
        let mut dst = [0u8; 4];
        Int16Complex::swap_slice(&src, &mut dst);
        assert_eq!(dst, [0x34, 0x12, 0x78, 0x56]);

        let src = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut dst = [0u8; 8];
        Float32Complex::swap_slice(&src, &mut dst);
        assert_eq!(dst, [4, 3, 2, 1, 8, 7, 6, 5]);
    }

    #[test]
    fn complex_value_swap_preserves_component_positions() {
        let mut c = Int16Complex {
            real: 0x1234,
            imag: 0x5678,
        };
        c.swap_bytes_value();
        assert_eq!((c.real, c.imag), (0x3412, 0x7856));

        let mut c = Float32Complex {
            real: 1.0,
            imag: 2.0,
        };
        c.swap_bytes_value();
        assert_eq!(c.real.to_bits(), 1.0f32.to_bits().swap_bytes());
        assert_eq!(c.imag.to_bits(), 2.0f32.to_bits().swap_bytes());
    }
}
