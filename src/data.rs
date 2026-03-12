//! Data blocks for voxel data access

use crate::{DecodeFromFile, EncodeToFile, Error, FileEndian, Mode, Packed4Bit, VoxelType};
use alloc::vec::Vec;

#[cfg(feature = "std")]
extern crate alloc;

/// Extended header - opaque metadata blob
///
/// This type provides read-only access to the extended header bytes.
/// No interpretation or endianness conversion is performed - it is
/// treated as an opaque byte sequence.
///
/// # API
/// - `len()` - length in bytes
/// - `is_empty()` - check if empty
/// - `as_bytes()` - read-only byte access
#[derive(Debug, Clone, Copy)]
pub struct ExtHeader<'a> {
    bytes: &'a [u8],
}

impl<'a> ExtHeader<'a> {
    /// Create a new ExtHeader from a byte slice
    #[inline]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Length of the extended header in bytes
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if the extended header is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Get read-only access to the raw bytes
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Mutable extended header - opaque metadata blob
///
/// This type provides mutable access to the extended header bytes.
/// No interpretation or endianness conversion is performed.
#[derive(Debug)]
pub struct ExtHeaderMut<'a> {
    bytes: &'a mut [u8],
}

impl<'a> ExtHeaderMut<'a> {
    /// Create a new ExtHeaderMut from a mutable byte slice
    #[inline]
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes }
    }

    /// Length of the extended header in bytes
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if the extended header is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Get read-only access to the raw bytes
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Get mutable access to the raw bytes
    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.bytes
    }
}

/// Iterator for individual 4-bit values from Packed4Bit data
///
/// This iterator provides efficient access to individual 4-bit values (0-15) from
/// packed byte data without the overhead of nested closures from flat_map.
pub(crate) struct Packed4BitValuesIterator<'a> {
    bytes: &'a [u8],
    byte_idx: usize,
    nibble: bool, // false = first nibble (lower 4 bits), true = second nibble (upper 4 bits)
    file_endian: FileEndian,
    remaining: usize,
}

impl<'a> Iterator for Packed4BitValuesIterator<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        let byte = self.bytes.get(self.byte_idx).copied()?;
        let packed = Packed4Bit::decode(self.file_endian, &[byte]);

        let value = if self.nibble {
            packed.second()
        } else {
            packed.first()
        };

        self.nibble = !self.nibble;
        self.byte_idx += if self.nibble { 1 } else { 0 };
        self.remaining -= 1;

        Some(value)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a> ExactSizeIterator for Packed4BitValuesIterator<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

/// Data block - voxel data with endianness-aware decoding
///
/// This type provides typed access to voxel data while maintaining
/// the raw file-endian bytes internally. All endianness conversion
/// happens only when decoding values.
///
/// # API
/// - `mode()` - data mode (type of voxels)
/// - `len_voxels()` - number of voxels
/// - `len_bytes()` - size in bytes
/// - `file_endian()` - file endianness
/// - `as_*()` - bulk decoding methods (e.g., as_f32(), as_i16())
/// - `as_bytes()` - read-only raw byte access
#[derive(Debug, Clone, Copy)]
pub struct DataBlock<'a> {
    bytes: &'a [u8],
    mode: Mode,
    file_endian: FileEndian,
    /// Expected number of voxels (from header dimensions).
    /// Used for Packed4Bit mode where buffer size may include padding.
    voxel_count: usize,
}

/// Helper functions for endianness-aware decoding (use DecodeFromFile trait)
#[inline]
fn decode_f32(bytes: &[u8], offset: usize, file_endian: FileEndian) -> f32 {
    f32::decode(file_endian, &bytes[offset..offset + 4])
}

#[inline]
fn decode_i16(bytes: &[u8], offset: usize, file_endian: FileEndian) -> i16 {
    i16::decode(file_endian, &bytes[offset..offset + 2])
}

#[inline]
fn decode_u16(bytes: &[u8], offset: usize, file_endian: FileEndian) -> u16 {
    u16::decode(file_endian, &bytes[offset..offset + 2])
}

impl<'a> DataBlock<'a> {
    /// Create a new DataBlock
    ///
    /// # Arguments
    /// * `bytes` - raw voxel data bytes (file-endian)
    /// * `mode` - data mode
    /// * `file_endian` - file endianness
    /// * `voxel_count` - expected number of voxels (from header dimensions)
    #[inline]
    pub fn new(bytes: &'a [u8], mode: Mode, file_endian: FileEndian, voxel_count: usize) -> Self {
        Self {
            bytes,
            mode,
            file_endian,
            voxel_count,
        }
    }

    /// Get the data mode
    #[inline]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Get the number of voxels
    #[inline]
    pub fn len_voxels(&self) -> usize {
        self.voxel_count
    }

    /// Get the size in bytes
    #[inline]
    pub fn len_bytes(&self) -> usize {
        self.bytes.len()
    }

    /// Get the file endianness
    #[inline]
    pub fn file_endian(&self) -> FileEndian {
        self.file_endian
    }

    /// Get read-only access to the raw bytes
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Unified zero-copy slice view (native endianness only)
    ///
    /// This is the primary method for accessing voxel data with zero-copy.
    /// Works only when the file endianness matches the native system endianness.
    ///
    /// # Type Parameters
    /// * `T` - The voxel type to view the data as (must implement VoxelType and Pod)
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode doesn't match T::MODE
    /// - Returns `Error::BufferTooSmall` if the byte length is less than expected
    /// - Returns `Error::WrongEndianness` if the file endianness doesn't match native
    /// - Returns `Error::MisalignedData` if the byte slice is not properly aligned
    #[inline]
    pub fn as_slice<T: VoxelType + bytemuck::Pod>(&self) -> Result<&[T], Error> {
        if !T::is_valid_mode(self.mode) {
            return Err(Error::InvalidMode);
        }

        let expected_byte_len = self.len_voxels() * T::SIZE;
        if self.bytes.len() < expected_byte_len {
            return Err(Error::BufferTooSmall {
                expected: expected_byte_len,
                got: self.bytes.len(),
            });
        }

        if !self.file_endian.is_native() {
            return Err(Error::WrongEndianness {
                file: self.file_endian,
                native: FileEndian::native(),
            });
        }

        bytemuck::try_cast_slice(&self.bytes[..expected_byte_len]).map_err(|e| {
            use bytemuck::PodCastError;
            match e {
                PodCastError::AlignmentMismatch => Error::MisalignedData {
                    required: core::mem::align_of::<T>(),
                    actual: self.bytes.as_ptr().align_offset(core::mem::align_of::<T>()),
                },
                PodCastError::SizeMismatch => Error::InvalidDimensions,
                _ => Error::InvalidDimensions,
            }
        })
    }

    /// Unified fallible iterator (works for all endianness)
    ///
    /// This is the secondary method for accessing voxel data when endianness
    /// conversion is needed. The iterator performs lazy decoding of each element.
    ///
    /// # Type Parameters
    /// * `T` - The voxel type to iterate over (must implement VoxelType)
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode doesn't match T::MODE
    #[inline]
    pub fn iter<T: VoxelType>(&self) -> Result<impl Iterator<Item = T> + '_, Error> {
        if !T::is_valid_mode(self.mode) {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.file_endian;
        let bytes = self.bytes;
        let len = self.len_voxels();

        Ok((0..len).map(move |i| {
            T::decode(file_endian, &bytes[i * T::SIZE..(i + 1) * T::SIZE])
        }))
    }

    /// Unified method to fill a pre-allocated buffer
    ///
    /// This is the utility method for copying data into a user-provided buffer.
    /// More efficient than `to_vec()` when you already have a buffer allocated.
    ///
    /// # Type Parameters
    /// * `T` - The voxel type to decode (must implement VoxelType and Pod)
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode doesn't match T::MODE
    /// - Returns `Error::BufferTooSmall` if the output buffer is too large for available data
    #[inline]
    pub fn copy_to<T: VoxelType + bytemuck::Pod>(&self, out: &mut [T]) -> Result<(), Error> {
        if !T::is_valid_mode(self.mode) {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        let expected_bytes = n * T::SIZE;
        if expected_bytes > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: expected_bytes,
                got: self.bytes.len(),
            });
        }

        // Fast path: native endianness with proper alignment - use copy_from_slice
        if self.file_endian.is_native() {
            if let Ok(slice) = bytemuck::try_cast_slice(&self.bytes[..expected_bytes]) {
                out.copy_from_slice(slice);
                return Ok(());
            }
        }

        // Fallback: element-by-element decoding
        for (i, dst) in out.iter_mut().enumerate() {
            *dst = T::decode(self.file_endian, &self.bytes[i * T::SIZE..(i + 1) * T::SIZE]);
        }

        Ok(())
    }

    /// Unified escape hatch: owned Vec allocation
    ///
    /// This is the fallback method when you need owned data and don't have
    /// a pre-allocated buffer. Prefer `as_slice()` for zero-copy access or
    /// `copy_to()` for filling existing buffers.
    ///
    /// # Type Parameters
    /// * `T` - The voxel type to decode (must implement VoxelType and Pod)
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode doesn't match T::MODE
    /// - Returns `Error::BufferTooSmall` if the byte length is less than expected
    #[inline]
    pub fn to_vec<T: VoxelType + bytemuck::Pod + Default>(&self) -> Result<Vec<T>, Error> {
        if !T::is_valid_mode(self.mode) {
            return Err(Error::InvalidMode);
        }

        let expected_byte_len = self.len_voxels() * T::SIZE;
        if self.bytes.len() < expected_byte_len {
            return Err(Error::BufferTooSmall {
                expected: expected_byte_len,
                got: self.bytes.len(),
            });
        }

        // Fast path: native endianness with proper alignment - use slice.to_vec()
        if self.file_endian.is_native() {
            if let Ok(slice) = bytemuck::try_cast_slice(&self.bytes[..expected_byte_len]) {
                return Ok(slice.to_vec());
            }
        }

        // Fallback: allocate and copy element-by-element
        let n = self.len_voxels();
        let mut result = Vec::with_capacity(n);
        result.resize(n, T::default());
        self.copy_to(&mut result)?;
        Ok(result)
    }

    /// Get a single f32 value at the specified voxel index
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Float32
    /// Returns Error::IndexOutOfBounds if index is out of bounds
    #[inline]
    pub fn get_f32(&self, index: usize) -> Result<f32, Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }
        let offset = index * 4;
        if offset + 4 > self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        Ok(decode_f32(self.bytes, offset, self.file_endian))
    }

    /// Create an iterator over f32 values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Float32
    #[inline]
    pub fn iter_f32(&self) -> Result<impl Iterator<Item = f32> + '_, Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }
        let len = self.len_voxels();
        let file_endian = self.file_endian;
        let bytes = self.bytes;

        Ok((0..len).map(move |i| decode_f32(bytes, i * 4, file_endian)))
    }

    /// Decode f32 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Float32
    /// Returns Error::BufferTooSmall if output buffer is too large for available data
    #[inline]
    pub fn read_f32_into(&self, out: &mut [f32]) -> Result<(), Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        if n * 4 > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: n * 4,
                got: self.bytes.len(),
            });
        }

        // Branchless endianness handling: check once outside loop
        match self.file_endian {
            FileEndian::LittleEndian => {
                for (i, chunk) in self.bytes.chunks_exact(4).enumerate().take(n) {
                    out[i] = f32::from_le_bytes(chunk.try_into().unwrap());
                }
            }
            FileEndian::BigEndian => {
                for (i, chunk) in self.bytes.chunks_exact(4).enumerate().take(n) {
                    out[i] = f32::from_be_bytes(chunk.try_into().unwrap());
                }
            }
        }

        Ok(())
    }

    /// Decode data as f32 values (allocates)
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32 (mode 2)
    /// Returns Error::InvalidDimensions if the byte length is not divisible by 4
    pub fn to_vec_f32(&self) -> Result<Vec<f32>, Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 4 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let n = self.bytes.len() / 4;
        let mut result = Vec::with_capacity(n);
        result.resize(n, 0.0f32);
        self.read_f32_into(&mut result)?;
        Ok(result)
    }

    /// Get zero-copy aligned f32 slice (native endianness only)
    ///
    /// This method provides a zero-copy view of the data as an f32 slice when the
    /// file endianness matches the native system endianness and the data is properly
    /// aligned. This is particularly useful for memory-mapped files where aligned access
    /// is critical for performance and correctness.
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode is not Float32
    /// - Returns `Error::InvalidDimensions` if the byte length is not divisible by 4
    /// - Returns `Error::WrongEndianness` if the file endianness doesn't match native
    /// - Returns `Error::MisalignedData` if the byte slice is not properly aligned for f32 access
    ///
    /// # Safety Note
    /// This method uses `bytemuck::try_cast_slice` which ensures memory alignment and
    /// prevents undefined behavior from unaligned access. The alignment check is critical
    /// for memory-mapped files which may start at arbitrary offsets.
    pub fn as_f32_slice(&self) -> Result<&[f32], Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 4 != 0 {
            return Err(Error::InvalidDimensions);
        }

        // Only allow zero-copy access for native endianness
        if !self.file_endian.is_native() {
            return Err(Error::WrongEndianness {
                file: self.file_endian,
                native: FileEndian::native(),
            });
        }

        // Use bytemuck to safely handle alignment requirements
        bytemuck::try_cast_slice(self.bytes).map_err(|e| {
            use bytemuck::PodCastError;
            match e {
                PodCastError::AlignmentMismatch => Error::MisalignedData {
                    required: core::mem::align_of::<f32>(),
                    actual: self.bytes.as_ptr().align_offset(core::mem::align_of::<f32>()),
                },
                _ => Error::InvalidDimensions,
            }
        })
    }

    /// Get a single i16 value at the specified voxel index
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int16
    /// Returns Error::IndexOutOfBounds if index is out of bounds
    #[inline]
    pub fn get_i16(&self, index: usize) -> Result<i16, Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }
        let offset = index * 2;
        if offset + 2 > self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        Ok(decode_i16(self.bytes, offset, self.file_endian))
    }

    /// Create an iterator over i16 values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int16
    #[inline]
    pub fn iter_i16(&self) -> Result<impl Iterator<Item = i16> + '_, Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }
        let len = self.len_voxels();
        let file_endian = self.file_endian;
        let bytes = self.bytes;

        Ok((0..len).map(move |i| decode_i16(bytes, i * 2, file_endian)))
    }

    /// Decode i16 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int16
    /// Returns Error::BufferTooSmall if output buffer is too large for available data
    #[inline]
    #[allow(clippy::needless_range_loop)] // Intentional: direct indexing for performance
    pub fn read_i16_into(&self, out: &mut [i16]) -> Result<(), Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        if n * 2 > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: n * 2,
                got: self.bytes.len(),
            });
        }

        for i in 0..n {
            out[i] = decode_i16(self.bytes, i * 2, self.file_endian);
        }

        Ok(())
    }

    /// Decode data as i16 values (allocates)
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16 (mode 1)
    /// Returns Error::InvalidDimensions if the byte length is not divisible by 2
    pub fn to_vec_i16(&self) -> Result<Vec<i16>, Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 2 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let n = self.bytes.len() / 2;
        let mut result: Vec<i16> = core::iter::repeat_n(0i16, n).collect();
        self.read_i16_into(&mut result)?;
        Ok(result)
    }

    /// Get zero-copy aligned i16 slice (native endianness only)
    ///
    /// Provides a zero-copy view of the data as an i16 slice when file endianness
    /// matches native system endianness and data is properly aligned.
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode is not Int16
    /// - Returns `Error::InvalidDimensions` if the byte length is not divisible by 2
    /// - Returns `Error::WrongEndianness` if the file endianness doesn't match native
    /// - Returns `Error::MisalignedData` if the byte slice is not properly aligned for i16 access
    pub fn as_i16_slice(&self) -> Result<&[i16], Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 2 != 0 {
            return Err(Error::InvalidDimensions);
        }

        if !self.file_endian.is_native() {
            return Err(Error::WrongEndianness {
                file: self.file_endian,
                native: FileEndian::native(),
            });
        }

        bytemuck::try_cast_slice(self.bytes).map_err(|e| {
            use bytemuck::PodCastError;
            match e {
                PodCastError::AlignmentMismatch => Error::MisalignedData {
                    required: core::mem::align_of::<i16>(),
                    actual: self.bytes.as_ptr().align_offset(core::mem::align_of::<i16>()),
                },
                _ => Error::InvalidDimensions,
            }
        })
    }

    /// Get a single u16 value at the specified voxel index
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Uint16
    /// Returns Error::IndexOutOfBounds if index is out of bounds
    #[inline]
    pub fn get_u16(&self, index: usize) -> Result<u16, Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }
        let offset = index * 2;
        if offset + 2 > self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        Ok(decode_u16(self.bytes, offset, self.file_endian))
    }

    /// Create an iterator over u16 values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Uint16
    #[inline]
    pub fn iter_u16(&self) -> Result<impl Iterator<Item = u16> + '_, Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }
        let len = self.len_voxels();
        let file_endian = self.file_endian;
        let bytes = self.bytes;

        Ok((0..len).map(move |i| decode_u16(bytes, i * 2, file_endian)))
    }

    /// Decode u16 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Uint16
    /// Returns Error::BufferTooSmall if output buffer is too large for available data
    #[inline]
    #[allow(clippy::needless_range_loop)] // Intentional: direct indexing for performance
    pub fn read_u16_into(&self, out: &mut [u16]) -> Result<(), Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        if n * 2 > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: n * 2,
                got: self.bytes.len(),
            });
        }

        for i in 0..n {
            out[i] = decode_u16(self.bytes, i * 2, self.file_endian);
        }

        Ok(())
    }

    /// Decode data as u16 values (allocates)
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Uint16 (mode 6)
    /// Returns Error::InvalidDimensions if the byte length is not divisible by 2
    pub fn to_vec_u16(&self) -> Result<Vec<u16>, Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 2 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let n = self.bytes.len() / 2;
        let mut result: Vec<u16> = core::iter::repeat_n(0u16, n).collect();
        self.read_u16_into(&mut result)?;
        Ok(result)
    }

    /// Get zero-copy aligned u16 slice (native endianness only)
    ///
    /// Provides a zero-copy view of the data as a u16 slice when file endianness
    /// matches native system endianness and data is properly aligned.
    ///
    /// # Errors
    /// - Returns `Error::InvalidMode` if the mode is not Uint16
    /// - Returns `Error::InvalidDimensions` if the byte length is not divisible by 2
    /// - Returns `Error::WrongEndianness` if the file endianness doesn't match native
    /// - Returns `Error::MisalignedData` if the byte slice is not properly aligned for u16 access
    pub fn as_u16_slice(&self) -> Result<&[u16], Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 2 != 0 {
            return Err(Error::InvalidDimensions);
        }

        if !self.file_endian.is_native() {
            return Err(Error::WrongEndianness {
                file: self.file_endian,
                native: FileEndian::native(),
            });
        }

        bytemuck::try_cast_slice(self.bytes).map_err(|e| {
            use bytemuck::PodCastError;
            match e {
                PodCastError::AlignmentMismatch => Error::MisalignedData {
                    required: core::mem::align_of::<u16>(),
                    actual: self.bytes.as_ptr().align_offset(core::mem::align_of::<u16>()),
                },
                _ => Error::InvalidDimensions,
            }
        })
    }

    /// Get a single i8 value at the specified voxel index
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int8
    /// Returns Error::IndexOutOfBounds if index is out of bounds
    #[inline]
    pub fn get_i8(&self, index: usize) -> Result<i8, Error> {
        if self.mode != Mode::Int8 {
            return Err(Error::InvalidMode);
        }
        if index >= self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        Ok(self.bytes[index] as i8)
    }

    /// Create an iterator over i8 values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int8
    #[inline]
    pub fn iter_i8(&self) -> Result<impl Iterator<Item = i8> + '_, Error> {
        if self.mode != Mode::Int8 {
            return Err(Error::InvalidMode);
        }
        Ok(self.bytes.iter().map(|&b| b as i8))
    }

    /// Decode i8 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int8
    /// Returns Error::BufferTooSmall if output buffer is too large for available data
    #[inline]
    pub fn read_i8_into(&self, out: &mut [i8]) -> Result<(), Error> {
        if self.mode != Mode::Int8 {
            return Err(Error::InvalidMode);
        }
        if out.len() > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: out.len(),
                got: self.bytes.len(),
            });
        }
        for (i, &byte) in self.bytes.iter().enumerate().take(out.len()) {
            out[i] = byte as i8;
        }
        Ok(())
    }

    // Include the rest of the methods...
    // For brevity, I'll continue with the remaining methods in the next part
}

// Continue DataBlock impl with remaining methods...
use crate::{Float32Complex, Int16Complex};

impl<'a> DataBlock<'a> {
    /// Get a single Int16Complex value at the specified voxel index
    #[inline]
    pub fn get_int16_complex(&self, index: usize) -> Result<Int16Complex, Error> {
        if self.mode != Mode::Int16Complex {
            return Err(Error::InvalidMode);
        }
        let offset = index * 4;
        if offset + 4 > self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        Ok(Int16Complex::decode(self.file_endian, &self.bytes[offset..offset + 4]))
    }

    /// Create an iterator over Int16Complex values
    #[inline]
    pub fn iter_int16_complex(&self) -> Result<impl Iterator<Item = Int16Complex> + '_, Error> {
        if self.mode != Mode::Int16Complex {
            return Err(Error::InvalidMode);
        }
        let file_endian = self.file_endian;
        Ok(self.bytes.chunks_exact(4).map(move |chunk| Int16Complex::decode(file_endian, chunk)))
    }

    /// Decode Int16Complex values into a pre-allocated buffer
    #[inline]
    pub fn read_int16_complex_into(&self, out: &mut [Int16Complex]) -> Result<(), Error> {
        if self.mode != Mode::Int16Complex {
            return Err(Error::InvalidMode);
        }
        let n = out.len();
        if n * 4 > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: n * 4,
                got: self.bytes.len(),
            });
        }
        for (i, chunk) in self.bytes.chunks_exact(4).take(n).enumerate() {
            out[i] = Int16Complex::decode(self.file_endian, chunk);
        }
        Ok(())
    }

    /// Get a single Float32Complex value at the specified voxel index
    #[inline]
    pub fn get_float32_complex(&self, index: usize) -> Result<Float32Complex, Error> {
        if self.mode != Mode::Float32Complex {
            return Err(Error::InvalidMode);
        }
        let offset = index * 8;
        if offset + 8 > self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        Ok(Float32Complex::decode(self.file_endian, &self.bytes[offset..offset + 8]))
    }

    /// Create an iterator over Float32Complex values
    #[inline]
    pub fn iter_float32_complex(&self) -> Result<impl Iterator<Item = Float32Complex> + '_, Error> {
        if self.mode != Mode::Float32Complex {
            return Err(Error::InvalidMode);
        }
        let file_endian = self.file_endian;
        Ok(self.bytes.chunks_exact(8).map(move |chunk| Float32Complex::decode(file_endian, chunk)))
    }

    /// Decode Float32Complex values into a pre-allocated buffer
    #[inline]
    pub fn read_float32_complex_into(&self, out: &mut [Float32Complex]) -> Result<(), Error> {
        if self.mode != Mode::Float32Complex {
            return Err(Error::InvalidMode);
        }
        let n = out.len();
        if n * 8 > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: n * 8,
                got: self.bytes.len(),
            });
        }
        for (i, chunk) in self.bytes.chunks_exact(8).take(n).enumerate() {
            out[i] = Float32Complex::decode(self.file_endian, chunk);
        }
        Ok(())
    }

    /// Get a single Packed4Bit value at the specified byte index
    #[inline]
    pub fn get_packed4bit(&self, index: usize) -> Result<Packed4Bit, Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }
        if index >= self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.bytes.len(),
            });
        }
        Ok(Packed4Bit::decode(self.file_endian, &[self.bytes[index]]))
    }

    /// Get a single 4-bit value at the specified voxel index (0-15)
    #[inline]
    pub fn get_packed4bit_value(&self, index: usize) -> Result<u8, Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }
        if index >= self.voxel_count {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.voxel_count,
            });
        }
        let byte_idx = index / 2;
        let nibble = index % 2;
        let packed = Packed4Bit::decode(self.file_endian, &[self.bytes[byte_idx]]);
        Ok(if nibble == 0 { packed.first() } else { packed.second() })
    }

    /// Create an iterator over Packed4Bit values (byte-oriented)
    #[inline]
    pub fn iter_packed4bit(&self) -> Result<impl Iterator<Item = Packed4Bit> + '_, Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }
        let file_endian = self.file_endian;
        Ok(self.bytes.iter().map(move |&b| Packed4Bit::decode(file_endian, &[b])))
    }

    /// Create an iterator over individual 4-bit values (0-15)
    #[inline]
    pub fn iter_packed4bit_values(&self) -> Result<impl Iterator<Item = u8> + '_, Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }

        Ok(Packed4BitValuesIterator {
            bytes: self.bytes,
            byte_idx: 0,
            nibble: false,
            file_endian: self.file_endian,
            remaining: self.voxel_count,
        })
    }

    /// Decode Packed4Bit values into a pre-allocated buffer
    #[inline]
    pub fn read_packed4bit_into(&self, out: &mut [Packed4Bit]) -> Result<(), Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }
        if out.len() > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: out.len(),
                got: self.bytes.len(),
            });
        }
        for (i, &byte) in self.bytes.iter().enumerate().take(out.len()) {
            out[i] = Packed4Bit::decode(self.file_endian, &[byte]);
        }
        Ok(())
    }

    /// Decode data as i8 values into a Vec
    pub fn to_vec_i8(&self) -> Result<Vec<i8>, Error> {
        if self.mode != Mode::Int8 {
            return Err(Error::InvalidMode);
        }

        let mut result = Vec::with_capacity(self.bytes.len());
        for byte in self.bytes {
            let value = i8::decode(self.file_endian, &[*byte]);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Int16Complex values into a Vec
    pub fn to_vec_i16_complex(&self) -> Result<Vec<Int16Complex>, Error> {
        if self.mode != Mode::Int16Complex {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 4 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let mut result = Vec::with_capacity(self.bytes.len() / 4);
        for chunk in self.bytes.chunks_exact(4) {
            let value = Int16Complex::decode(self.file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Float32Complex values into a Vec
    pub fn to_vec_f32_complex(&self) -> Result<Vec<Float32Complex>, Error> {
        if self.mode != Mode::Float32Complex {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 8 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let mut result = Vec::with_capacity(self.bytes.len() / 8);
        for chunk in self.bytes.chunks_exact(8) {
            let value = Float32Complex::decode(self.file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Packed4Bit values into a Vec
    pub fn to_vec_packed4bit(&self) -> Result<Vec<Packed4Bit>, Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }

        let mut result = Vec::with_capacity(self.bytes.len());
        for byte in self.bytes {
            let value = Packed4Bit::decode(self.file_endian, &[*byte]);
            result.push(value);
        }

        Ok(result)
    }

    // f16 methods
    #[cfg(feature = "f16")]
    pub fn get_f16(&self, index: usize) -> Result<half::f16, Error> {
        if self.mode != Mode::Float16 {
            return Err(Error::InvalidMode);
        }
        let offset = index * 2;
        if offset + 2 > self.bytes.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len_voxels(),
            });
        }
        let bits = match self.file_endian {
            FileEndian::LittleEndian => u16::from_le_bytes([self.bytes[offset], self.bytes[offset + 1]]),
            FileEndian::BigEndian => u16::from_be_bytes([self.bytes[offset], self.bytes[offset + 1]]),
        };
        Ok(half::f16::from_bits(bits))
    }

    #[cfg(feature = "f16")]
    pub fn iter_f16(&self) -> Result<impl Iterator<Item = half::f16> + '_, Error> {
        if self.mode != Mode::Float16 {
            return Err(Error::InvalidMode);
        }
        let file_endian = self.file_endian;
        let bytes = self.bytes;
        Ok(bytes.chunks_exact(2).map(move |chunk| {
            let bits = match file_endian {
                FileEndian::LittleEndian => u16::from_le_bytes([chunk[0], chunk[1]]),
                FileEndian::BigEndian => u16::from_be_bytes([chunk[0], chunk[1]]),
            };
            half::f16::from_bits(bits)
        }))
    }

    #[cfg(feature = "f16")]
    pub fn read_f16_into(&self, out: &mut [half::f16]) -> Result<(), Error> {
        if self.mode != Mode::Float16 {
            return Err(Error::InvalidMode);
        }
        let n = out.len();
        if n * 2 > self.bytes.len() {
            return Err(Error::BufferTooSmall {
                expected: n * 2,
                got: self.bytes.len(),
            });
        }
        for (i, chunk) in self.bytes.chunks_exact(2).take(n).enumerate() {
            let bits = match self.file_endian {
                FileEndian::LittleEndian => u16::from_le_bytes([chunk[0], chunk[1]]),
                FileEndian::BigEndian => u16::from_be_bytes([chunk[0], chunk[1]]),
            };
            out[i] = half::f16::from_bits(bits);
        }
        Ok(())
    }

    #[cfg(feature = "f16")]
    pub fn to_vec_f16(&self) -> Result<Vec<half::f16>, Error> {
        if self.mode != Mode::Float16 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 2 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let mut result = Vec::with_capacity(self.bytes.len() / 2);

        for chunk in self.bytes.chunks_exact(2) {
            let bits = match self.file_endian {
                FileEndian::LittleEndian => u16::from_le_bytes([chunk[0], chunk[1]]),
                FileEndian::BigEndian => u16::from_be_bytes([chunk[0], chunk[1]]),
            };
            result.push(half::f16::from_bits(bits));
        }

        Ok(result)
    }
}

/// Mutable data block - voxel data with endianness-aware encoding
#[derive(Debug)]
pub struct DataBlockMut<'a> {
    bytes: &'a mut [u8],
    mode: Mode,
    file_endian: FileEndian,
    voxel_count: usize,
}

impl<'a> DataBlockMut<'a> {
    /// Create a new DataBlockMut
    #[inline]
    pub fn new(bytes: &'a mut [u8], mode: Mode, file_endian: FileEndian, voxel_count: usize) -> Self {
        Self {
            bytes,
            mode,
            file_endian,
            voxel_count,
        }
    }

    /// Get the data mode
    #[inline]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Get the number of voxels
    #[inline]
    pub fn len_voxels(&self) -> usize {
        self.voxel_count
    }

    /// Get the size in bytes
    #[inline]
    pub fn len_bytes(&self) -> usize {
        self.bytes.len()
    }

    /// Get the file endianness
    #[inline]
    pub fn file_endian(&self) -> FileEndian {
        self.file_endian
    }

    /// Get read-only access to the raw bytes
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Get mutable access to the raw bytes
    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.bytes
    }

    /// Get a read-only DataBlock view for reading data
    #[inline]
    pub fn as_data_block(&self) -> DataBlock<'_> {
        DataBlock::new(self.bytes, self.mode, self.file_endian, self.voxel_count)
    }

    /// Encode f32 values to data
    pub fn set_f32(&mut self, values: &[f32]) -> Result<(), Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }

        if values.len() * 4 != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i * 4..i * 4 + 4]);
        }

        Ok(())
    }

    /// Encode i16 values to data
    pub fn set_i16(&mut self, values: &[i16]) -> Result<(), Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }

        if values.len() * 2 != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i * 2..i * 2 + 2]);
        }

        Ok(())
    }

    /// Encode u16 values to data
    pub fn set_u16(&mut self, values: &[u16]) -> Result<(), Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }

        if values.len() * 2 != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i * 2..i * 2 + 2]);
        }

        Ok(())
    }

    /// Encode i8 values to data
    pub fn set_i8(&mut self, values: &[i8]) -> Result<(), Error> {
        if self.mode != Mode::Int8 {
            return Err(Error::InvalidMode);
        }

        if values.len() != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i..i + 1]);
        }

        Ok(())
    }

    /// Encode Int16Complex values to data
    pub fn set_int16_complex(&mut self, values: &[Int16Complex]) -> Result<(), Error> {
        if self.mode != Mode::Int16Complex {
            return Err(Error::InvalidMode);
        }

        if values.len() * 4 != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i * 4..i * 4 + 4]);
        }

        Ok(())
    }

    /// Encode Float32Complex values to data
    pub fn set_float32_complex(&mut self, values: &[Float32Complex]) -> Result<(), Error> {
        if self.mode != Mode::Float32Complex {
            return Err(Error::InvalidMode);
        }

        if values.len() * 8 != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i * 8..i * 8 + 8]);
        }

        Ok(())
    }

    /// Encode Packed4Bit values to data
    pub fn set_packed4bit(&mut self, values: &[Packed4Bit]) -> Result<(), Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }

        if values.len() != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            value.encode(self.file_endian, &mut self.bytes[i..i + 1]);
        }

        Ok(())
    }

    #[cfg(feature = "f16")]
    /// Encode f16 values to data
    pub fn set_f16(&mut self, values: &[half::f16]) -> Result<(), Error> {
        if self.mode != Mode::Float16 {
            return Err(Error::InvalidMode);
        }

        if values.len() * 2 != self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        for (i, &value) in values.iter().enumerate() {
            let bits = value.to_bits();
            let bytes = match self.file_endian {
                FileEndian::LittleEndian => bits.to_le_bytes(),
                FileEndian::BigEndian => bits.to_be_bytes(),
            };
            self.bytes[i * 2..i * 2 + 2].copy_from_slice(&bytes);
        }

        Ok(())
    }
}
