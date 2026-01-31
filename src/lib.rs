//! # MRC File Format Library
//!
//! This crate provides a safe, efficient, and endian-correct implementation for reading
//! and writing MRC (Medical Research Council) files, which are commonly used in
//! cryo-electron microscopy and structural biology.
//!
//! ## Memory Model
//!
//! This crate strictly separates the three components of an MRC file:
//!
//! ```text
//! File layout:  | 1024 bytes | NSYMBT bytes | data_size bytes |
//!               | Header     | ExtHeader    | VoxelData       |
//!
//! Memory model: | Header     | ExtHeader    | VoxelData       |
//!               | (decoded)  | (raw bytes)  | (raw bytes)     |
//!               | native-end| opaque       | file-endian     |
//! ```
//!
//! - **Header** (1024 bytes): Always decoded on load, always native-endian in memory
//! - **Extended header** (NSYMBT bytes): Opaque bytes, no endianness conversion
//! - **Voxel data** (data_size bytes): Raw bytes in file-endian, decoded lazily on access
//!
//! Endianness conversion occurs **only** when decoding or encoding typed numeric values
//! through the `DecodeFromFile` and `EncodeToFile` traits. This ensures zero-copy mmap
//! views and prevents accidental endian corruption.
//!
//! ## Features
//!
//! - `std`: Standard library support for file I/O
//! - `mmap`: Memory-mapped file support for zero-copy access
//! - `f16`: Half-precision floating point support
//!
//! ## Safety
//!
//! All operations are memory-safe. The crate uses no unsafe code for data access,
//! and all endianness conversions are performed through safe, type-checked APIs.
//!
//! ## Endianness Policy
//!
//! This crate enforces a simple and safe endianness model:
//!
//! - All newly created MRC files are written in little-endian format.
//! - Existing MRC files are read and modified using their declared file endianness.
//! - Endianness is handled internally during numeric decode/encode.
//! - Users never need to reason about byte order.
//!
//! This guarantees compatibility with the MRC2014 ecosystem while supporting
//! cross-platform reading, writing, memory-mapped access, and streaming updates.

#![no_std]
#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "f16")]
extern crate half;

#[cfg(feature = "std")]
extern crate std;

use alloc::vec::Vec;

mod header;
mod mode;
mod view;

#[cfg(test)]
#[path = "../test/tests.rs"]
mod tests;

pub use header::Header;
pub use mode::Mode;
pub use view::{MrcView, MrcViewMut};

/// Endianness of MRC file data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEndian {
    LittleEndian,
    BigEndian,
}

impl FileEndian {
    /// Detect file endianness from MACHST machine stamp
    ///
    /// According to MRC2014 spec:
    /// - 0x44 0x44 0x00 0x00 indicates little-endian
    /// - 0x11 0x11 0x00 0x00 indicates big-endian
    ///
    /// # Note
    /// Endianness is determined solely from the first two bytes of MACHST.
    /// The last two bytes (padding) are ignored for endianness detection,
    /// but a warning is emitted if they contain non-zero values.
    pub fn from_machst(machst: &[u8; 4]) -> Self {
        // Check first two bytes (bytes 213-214 in header)
        // 0x44 = 'D' in ASCII, indicates little-endian
        // 0x11 indicates big-endian
        let endian = if machst[0] == 0x44 && machst[1] == 0x44 {
            FileEndian::LittleEndian
        } else if machst[0] == 0x11 && machst[1] == 0x11 {
            FileEndian::BigEndian
        } else {
            // Default to little-endian for unknown values
            // (most common in practice)
            FileEndian::LittleEndian
        };

        // Warn about non-standard padding bytes (bytes 2-3)
        #[cfg(feature = "std")]
        {
            if machst[2] != 0 || machst[3] != 0 {
                std::eprintln!(
                    "Warning: Non-standard MACHST padding bytes: {:02X} {:02X} {:02X} {:02X}",
                    machst[0],
                    machst[1],
                    machst[2],
                    machst[3]
                );
            }
        }

        endian
    }

    /// Convert FileEndian to MACHST bytes
    ///
    /// Returns the 4-byte machine stamp encoding for this endianness.
    pub fn to_machst(self) -> [u8; 4] {
        match self {
            FileEndian::LittleEndian => [0x44, 0x44, 0x00, 0x00],
            FileEndian::BigEndian => [0x11, 0x11, 0x00, 0x00],
        }
    }

    /// Get native system endianness
    #[inline]
    pub fn native() -> Self {
        #[cfg(target_endian = "little")]
        {
            FileEndian::LittleEndian
        }
        #[cfg(target_endian = "big")]
        {
            FileEndian::BigEndian
        }
    }

    /// Check if this is the native endianness
    #[inline]
    pub fn is_native(self) -> bool {
        self == Self::native()
    }
}

/// Output endianness for writing MRC files
///
/// Controls how data is encoded when writing to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEndian {
    /// Use native system endianness (default)
    Native,
    /// Force little-endian output
    LittleEndian,
    /// Force big-endian output
    BigEndian,
}

impl OutputEndian {
    /// Get the actual FileEndian for this output setting
    #[inline]
    pub fn as_file_endian(self) -> FileEndian {
        match self {
            OutputEndian::Native => FileEndian::native(),
            OutputEndian::LittleEndian => FileEndian::LittleEndian,
            OutputEndian::BigEndian => FileEndian::BigEndian,
        }
    }

    /// Get the MACHST bytes for this output setting
    #[inline]
    pub fn machst_bytes(self) -> [u8; 4] {
        match self.as_file_endian() {
            FileEndian::LittleEndian => [0x44, 0x44, 0x00, 0x00],
            FileEndian::BigEndian => [0x11, 0x11, 0x00, 0x00],
        }
    }
}

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
}

impl<'a> DataBlock<'a> {
    /// Create a new DataBlock
    ///
    /// # Arguments
    /// * `bytes` - raw voxel data bytes (file-endian)
    /// * `mode` - data mode
    /// * `file_endian` - file endianness
    #[inline]
    pub fn new(bytes: &'a [u8], mode: Mode, file_endian: FileEndian) -> Self {
        Self {
            bytes,
            mode,
            file_endian,
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
        match self.mode {
            Mode::Packed4Bit => self.bytes.len() * 2, // 2 voxels per byte (4 bits each)
            _ => {
                let byte_size = self.mode.byte_size();
                if byte_size == 0 {
                    return 0;
                }
                self.bytes.len() / byte_size
            }
        }
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

    /// Get a single f32 value at the specified voxel index
    ///
    /// # Panics
    /// Panics if index is out of bounds
    #[inline]
    pub fn get_f32(&self, index: usize) -> f32 {
        assert!(self.mode == Mode::Float32, "Mode must be Float32");
        let offset = index * 4;
        debug_assert!(offset + 4 <= self.bytes.len());

        if self.file_endian.is_native() {
            // Fast path: native endian
            let arr: [u8; 4] = [
                self.bytes[offset],
                self.bytes[offset + 1],
                self.bytes[offset + 2],
                self.bytes[offset + 3],
            ];
            #[cfg(target_endian = "little")]
            return f32::from_le_bytes(arr);
            #[cfg(target_endian = "big")]
            return f32::from_be_bytes(arr);
        } else {
            // Byte swap needed
            let arr: [u8; 4] = [
                self.bytes[offset],
                self.bytes[offset + 1],
                self.bytes[offset + 2],
                self.bytes[offset + 3],
            ];
            match self.file_endian {
                FileEndian::LittleEndian => f32::from_le_bytes(arr),
                FileEndian::BigEndian => f32::from_be_bytes(arr),
            }
        }
    }

    /// Create an iterator over f32 values
    ///
    /// # Panics
    /// Panics if mode is not Float32
    #[inline]
    pub fn iter_f32(&self) -> impl Iterator<Item = f32> + '_ {
        assert!(self.mode == Mode::Float32, "Mode must be Float32");
        let len = self.len_voxels();
        let file_endian = self.file_endian;
        let bytes = self.bytes;

        (0..len).map(move |i| {
            let offset = i * 4;
            if file_endian.is_native() {
                let arr: [u8; 4] = [
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ];
                #[cfg(target_endian = "little")]
                return f32::from_le_bytes(arr);
                #[cfg(target_endian = "big")]
                return f32::from_be_bytes(arr);
            } else {
                let arr: [u8; 4] = [
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ];
                match file_endian {
                    FileEndian::LittleEndian => f32::from_le_bytes(arr),
                    FileEndian::BigEndian => f32::from_be_bytes(arr),
                }
            }
        })
    }

    /// Decode f32 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Float32
    /// Returns Error::InvalidDimensions if output buffer is too small
    #[inline]
    #[allow(clippy::needless_range_loop)] // Intentional: direct indexing for performance
    pub fn read_f32_into(&self, out: &mut [f32]) -> Result<(), Error> {
        if self.mode != Mode::Float32 {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        if n * 4 > self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        if self.file_endian.is_native() {
            // Fast native-endian path
            #[cfg(target_endian = "little")]
            for i in 0..n {
                let offset = i * 4;
                let arr: [u8; 4] = [
                    self.bytes[offset],
                    self.bytes[offset + 1],
                    self.bytes[offset + 2],
                    self.bytes[offset + 3],
                ];
                out[i] = f32::from_le_bytes(arr);
            }
            #[cfg(target_endian = "big")]
            for i in 0..n {
                let offset = i * 4;
                let arr: [u8; 4] = [
                    self.bytes[offset],
                    self.bytes[offset + 1],
                    self.bytes[offset + 2],
                    self.bytes[offset + 3],
                ];
                out[i] = f32::from_be_bytes(arr);
            }
        } else {
            // Byte swap needed
            for i in 0..n {
                let offset = i * 4;
                let arr: [u8; 4] = [
                    self.bytes[offset],
                    self.bytes[offset + 1],
                    self.bytes[offset + 2],
                    self.bytes[offset + 3],
                ];
                out[i] = match self.file_endian {
                    FileEndian::LittleEndian => f32::from_le_bytes(arr),
                    FileEndian::BigEndian => f32::from_be_bytes(arr),
                };
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
        let mut result: Vec<f32> = core::iter::repeat_n(0.0f32, n).collect();
        self.read_f32_into(&mut result)?;
        Ok(result)
    }

    /// Get a single i16 value at the specified voxel index
    ///
    /// # Panics
    /// Panics if index is out of bounds
    #[inline]
    pub fn get_i16(&self, index: usize) -> i16 {
        assert!(self.mode == Mode::Int16, "Mode must be Int16");
        let offset = index * 2;
        debug_assert!(offset + 2 <= self.bytes.len());

        if self.file_endian.is_native() {
            let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
            #[cfg(target_endian = "little")]
            return i16::from_le_bytes(arr);
            #[cfg(target_endian = "big")]
            return i16::from_be_bytes(arr);
        } else {
            let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
            match self.file_endian {
                FileEndian::LittleEndian => i16::from_le_bytes(arr),
                FileEndian::BigEndian => i16::from_be_bytes(arr),
            }
        }
    }

    /// Create an iterator over i16 values
    ///
    /// # Panics
    /// Panics if mode is not Int16
    #[inline]
    pub fn iter_i16(&self) -> impl Iterator<Item = i16> + '_ {
        assert!(self.mode == Mode::Int16, "Mode must be Int16");
        let len = self.len_voxels();
        let file_endian = self.file_endian;
        let bytes = self.bytes;

        (0..len).map(move |i| {
            let offset = i * 2;
            if file_endian.is_native() {
                let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
                #[cfg(target_endian = "little")]
                return i16::from_le_bytes(arr);
                #[cfg(target_endian = "big")]
                return i16::from_be_bytes(arr);
            } else {
                let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
                match file_endian {
                    FileEndian::LittleEndian => i16::from_le_bytes(arr),
                    FileEndian::BigEndian => i16::from_be_bytes(arr),
                }
            }
        })
    }

    /// Decode i16 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int16
    /// Returns Error::InvalidDimensions if output buffer is too small
    #[inline]
    #[allow(clippy::needless_range_loop)] // Intentional: direct indexing for performance
    pub fn read_i16_into(&self, out: &mut [i16]) -> Result<(), Error> {
        if self.mode != Mode::Int16 {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        if n * 2 > self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        if self.file_endian.is_native() {
            #[cfg(target_endian = "little")]
            for i in 0..n {
                let offset = i * 2;
                let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
                out[i] = i16::from_le_bytes(arr);
            }
            #[cfg(target_endian = "big")]
            for i in 0..n {
                let offset = i * 2;
                let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
                out[i] = i16::from_be_bytes(arr);
            }
        } else {
            for i in 0..n {
                let offset = i * 2;
                let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
                out[i] = match self.file_endian {
                    FileEndian::LittleEndian => i16::from_le_bytes(arr),
                    FileEndian::BigEndian => i16::from_be_bytes(arr),
                };
            }
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

    /// Get a single u16 value at the specified voxel index
    ///
    /// # Panics
    /// Panics if index is out of bounds
    #[inline]
    pub fn get_u16(&self, index: usize) -> u16 {
        assert!(self.mode == Mode::Uint16, "Mode must be Uint16");
        let offset = index * 2;
        debug_assert!(offset + 2 <= self.bytes.len());

        if self.file_endian.is_native() {
            let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
            #[cfg(target_endian = "little")]
            return u16::from_le_bytes(arr);
            #[cfg(target_endian = "big")]
            return u16::from_be_bytes(arr);
        } else {
            let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
            match self.file_endian {
                FileEndian::LittleEndian => u16::from_le_bytes(arr),
                FileEndian::BigEndian => u16::from_be_bytes(arr),
            }
        }
    }

    /// Create an iterator over u16 values
    ///
    /// # Panics
    /// Panics if mode is not Uint16
    #[inline]
    pub fn iter_u16(&self) -> impl Iterator<Item = u16> + '_ {
        assert!(self.mode == Mode::Uint16, "Mode must be Uint16");
        let len = self.len_voxels();
        let file_endian = self.file_endian;
        let bytes = self.bytes;

        (0..len).map(move |i| {
            let offset = i * 2;
            if file_endian.is_native() {
                let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
                #[cfg(target_endian = "little")]
                return u16::from_le_bytes(arr);
                #[cfg(target_endian = "big")]
                return u16::from_be_bytes(arr);
            } else {
                let arr: [u8; 2] = [bytes[offset], bytes[offset + 1]];
                match file_endian {
                    FileEndian::LittleEndian => u16::from_le_bytes(arr),
                    FileEndian::BigEndian => u16::from_be_bytes(arr),
                }
            }
        })
    }

    /// Decode u16 values into a pre-allocated buffer
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Uint16
    /// Returns Error::InvalidDimensions if output buffer is too small
    #[inline]
    #[allow(clippy::needless_range_loop)] // Intentional: direct indexing for performance
    pub fn read_u16_into(&self, out: &mut [u16]) -> Result<(), Error> {
        if self.mode != Mode::Uint16 {
            return Err(Error::InvalidMode);
        }

        let n = out.len();
        if n * 2 > self.bytes.len() {
            return Err(Error::InvalidDimensions);
        }

        if self.file_endian.is_native() {
            #[cfg(target_endian = "little")]
            for i in 0..n {
                let offset = i * 2;
                let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
                out[i] = u16::from_le_bytes(arr);
            }
            #[cfg(target_endian = "big")]
            for i in 0..n {
                let offset = i * 2;
                let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
                out[i] = u16::from_be_bytes(arr);
            }
        } else {
            for i in 0..n {
                let offset = i * 2;
                let arr: [u8; 2] = [self.bytes[offset], self.bytes[offset + 1]];
                out[i] = match self.file_endian {
                    FileEndian::LittleEndian => u16::from_le_bytes(arr),
                    FileEndian::BigEndian => u16::from_be_bytes(arr),
                };
            }
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

    /// Decode data as i8 values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int8 (mode 0)
    pub fn as_i8(&self) -> Result<Vec<i8>, Error> {
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

    /// Decode data as Int16Complex values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16Complex (mode 3)
    /// Returns Error::InvalidDimensions if the byte length is not divisible by 4
    pub fn as_int16_complex(&self) -> Result<Vec<Int16Complex>, Error> {
        if self.mode != Mode::Int16Complex {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 4 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let mut result = Vec::with_capacity(self.bytes.len() / 4);
        let chunks: Vec<_> = self.bytes.chunks_exact(4).collect();

        for chunk in chunks {
            let value = Int16Complex::decode(self.file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Float32Complex values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32Complex (mode 4)
    /// Returns Error::InvalidDimensions if the byte length is not divisible by 8
    pub fn as_float32_complex(&self) -> Result<Vec<Float32Complex>, Error> {
        if self.mode != Mode::Float32Complex {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 8 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let mut result = Vec::with_capacity(self.bytes.len() / 8);
        let chunks: Vec<_> = self.bytes.chunks_exact(8).collect();

        for chunk in chunks {
            let value = Float32Complex::decode(self.file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Packed4Bit values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Packed4Bit (mode 101)
    pub fn as_packed4bit(&self) -> Result<Vec<Packed4Bit>, Error> {
        if self.mode != Mode::Packed4Bit {
            return Err(Error::InvalidMode);
        }

        let mut result = Vec::with_capacity(self.bytes.len() * 2);
        for byte in self.bytes {
            let value = Packed4Bit::decode(self.file_endian, &[*byte]);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as f16 values
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float16 (mode 12)
    /// Returns Error::InvalidDimensions if the byte length is not divisible by 2
    /// Returns Error::UnsupportedMode if the f16 feature is not enabled
    #[cfg(feature = "f16")]
    pub fn as_f16(&self) -> Result<Vec<half::f16>, Error> {
        if self.mode != Mode::Float16 {
            return Err(Error::InvalidMode);
        }

        if self.bytes.len() % 2 != 0 {
            return Err(Error::InvalidDimensions);
        }

        let mut result = Vec::with_capacity(self.bytes.len() / 2);
        let chunks: Vec<_> = self.bytes.chunks_exact(2).collect();

        for chunk in chunks {
            let value = half::f16::decode(self.file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }
}

/// Mutable data block - voxel data with endianness-aware encoding
///
/// This type provides typed write access to voxel data while maintaining
/// the raw file-endian bytes internally. All endianness conversion
/// happens only when encoding values.
///
/// # API
/// - `mode()` - data mode (type of voxels)
/// - `len_voxels()` - number of voxels
/// - `len_bytes()` - size in bytes
/// - `file_endian()` - file endianness
/// - `set_*()` - bulk encoding methods (e.g., set_f32(), set_i16())
/// - `as_bytes()` - read-only raw byte access
/// - `as_bytes_mut()` - mutable raw byte access
#[derive(Debug)]
pub struct DataBlockMut<'a> {
    bytes: &'a mut [u8],
    mode: Mode,
    file_endian: FileEndian,
}

impl<'a> DataBlockMut<'a> {
    /// Create a new DataBlockMut
    ///
    /// # Arguments
    /// * `bytes` - mutable voxel data bytes (file-endian)
    /// * `mode` - data mode
    /// * `file_endian` - file endianness
    #[inline]
    pub fn new(bytes: &'a mut [u8], mode: Mode, file_endian: FileEndian) -> Self {
        Self {
            bytes,
            mode,
            file_endian,
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
        match self.mode {
            Mode::Packed4Bit => self.bytes.len() * 2, // 2 voxels per byte (4 bits each)
            _ => {
                let byte_size = self.mode.byte_size();
                if byte_size == 0 {
                    return 0;
                }
                self.bytes.len() / byte_size
            }
        }
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

    /// Encode f32 values to data
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32 (mode 2)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16 (mode 1)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Uint16 (mode 6)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int8 (mode 0)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16Complex (mode 3)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32Complex (mode 4)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Packed4Bit (mode 101)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
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
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float16 (mode 12)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn set_f16(&mut self, values: &[half::f16]) -> Result<(), Error> {
        if self.mode != Mode::Float16 {
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
}

/// Decode typed values from raw file bytes with correct endianness
///
/// This is the ONLY place where file endianness conversion happens.
/// Raw bytes are always in file endian, returned values are always native endian.
pub trait DecodeFromFile: Sized {
    /// Size of this type in bytes
    const SIZE: usize;

    /// Decode from raw bytes, converting from file endian to native endian
    fn decode(file_endian: FileEndian, bytes: &[u8]) -> Self;
}

/// Encode typed values to raw file bytes with correct endianness
///
/// This is the ONLY place where file endianness conversion happens.
/// Input values are always native endian, output bytes are always file endian.
pub trait EncodeToFile {
    /// Size of this type in bytes
    const SIZE: usize;

    /// Encode to raw bytes, converting from native endian to file endian
    fn encode(self, file_endian: FileEndian, out: &mut [u8]);
}

// Implementations for primitive types used in MRC files

impl DecodeFromFile for i32 {
    const SIZE: usize = 4;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 4] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => i32::from_le_bytes(arr),
            FileEndian::BigEndian => i32::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for i32 {
    const SIZE: usize = 4;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for f32 {
    const SIZE: usize = 4;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 4] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => f32::from_le_bytes(arr),
            FileEndian::BigEndian => f32::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for f32 {
    const SIZE: usize = 4;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for i16 {
    const SIZE: usize = 2;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 2] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => i16::from_le_bytes(arr),
            FileEndian::BigEndian => i16::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for i16 {
    const SIZE: usize = 2;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for u16 {
    const SIZE: usize = 2;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 2] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for u16 {
    const SIZE: usize = 2;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for i8 {
    const SIZE: usize = 1;

    fn decode(_e: FileEndian, b: &[u8]) -> Self {
        b[0] as i8
    }
}

impl EncodeToFile for i8 {
    const SIZE: usize = 1;

    fn encode(self, _e: FileEndian, out: &mut [u8]) {
        out[0] = self as u8;
    }
}

// Complex number types for MRC modes 3 and 4

/// Complex number with 16-bit integer components (Mode 3)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Int16Complex {
    pub real: i16,
    pub imag: i16,
}

impl DecodeFromFile for Int16Complex {
    const SIZE: usize = 4;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let real_arr: [u8; 2] = b[0..2].try_into().unwrap();
        let imag_arr: [u8; 2] = b[2..4].try_into().unwrap();
        Self {
            real: match e {
                FileEndian::LittleEndian => i16::from_le_bytes(real_arr),
                FileEndian::BigEndian => i16::from_be_bytes(real_arr),
            },
            imag: match e {
                FileEndian::LittleEndian => i16::from_le_bytes(imag_arr),
                FileEndian::BigEndian => i16::from_be_bytes(imag_arr),
            },
        }
    }
}

impl EncodeToFile for Int16Complex {
    const SIZE: usize = 4;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let real_bytes = match e {
            FileEndian::LittleEndian => self.real.to_le_bytes(),
            FileEndian::BigEndian => self.real.to_be_bytes(),
        };
        let imag_bytes = match e {
            FileEndian::LittleEndian => self.imag.to_le_bytes(),
            FileEndian::BigEndian => self.imag.to_be_bytes(),
        };
        out[0..2].copy_from_slice(&real_bytes);
        out[2..4].copy_from_slice(&imag_bytes);
    }
}

/// Complex number with 32-bit float components (Mode 4)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Float32Complex {
    pub real: f32,
    pub imag: f32,
}

impl DecodeFromFile for Float32Complex {
    const SIZE: usize = 8;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let real_arr: [u8; 4] = b[0..4].try_into().unwrap();
        let imag_arr: [u8; 4] = b[4..8].try_into().unwrap();
        Self {
            real: match e {
                FileEndian::LittleEndian => f32::from_le_bytes(real_arr),
                FileEndian::BigEndian => f32::from_be_bytes(real_arr),
            },
            imag: match e {
                FileEndian::LittleEndian => f32::from_le_bytes(imag_arr),
                FileEndian::BigEndian => f32::from_be_bytes(imag_arr),
            },
        }
    }
}

impl EncodeToFile for Float32Complex {
    const SIZE: usize = 8;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let real_bytes = match e {
            FileEndian::LittleEndian => self.real.to_le_bytes(),
            FileEndian::BigEndian => self.real.to_be_bytes(),
        };
        let imag_bytes = match e {
            FileEndian::LittleEndian => self.imag.to_le_bytes(),
            FileEndian::BigEndian => self.imag.to_be_bytes(),
        };
        out[0..4].copy_from_slice(&real_bytes);
        out[4..8].copy_from_slice(&imag_bytes);
    }
}

// Packed 4-bit data (Mode 101)
// Two 4-bit values are packed into a single byte

/// Packed 4-bit values (Mode 101)
///
/// Two 4-bit values (0-15) are packed into a single byte.
/// The lower 4 bits contain the first value, the upper 4 bits contain the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packed4Bit {
    pub values: [u8; 2], // Each value is 0-15
}

impl Packed4Bit {
    /// Create a new packed 4-bit value
    pub fn new(first: u8, second: u8) -> Self {
        debug_assert!(first <= 15, "First value must be 0-15");
        debug_assert!(second <= 15, "Second value must be 0-15");
        Self {
            values: [first, second],
        }
    }

    /// Get the first (lower) 4-bit value
    pub fn first(&self) -> u8 {
        self.values[0]
    }

    /// Get the second (upper) 4-bit value
    pub fn second(&self) -> u8 {
        self.values[1]
    }
}

impl DecodeFromFile for Packed4Bit {
    const SIZE: usize = 1;

    fn decode(_e: FileEndian, b: &[u8]) -> Self {
        let byte = b[0];
        Self {
            values: [byte & 0x0F, (byte >> 4) & 0x0F],
        }
    }
}

impl EncodeToFile for Packed4Bit {
    const SIZE: usize = 1;

    fn encode(self, _e: FileEndian, out: &mut [u8]) {
        out[0] = self.values[0] | (self.values[1] << 4);
    }
}

// Optional f16 support (Mode 12)

#[cfg(feature = "f16")]
impl DecodeFromFile for half::f16 {
    const SIZE: usize = 2;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 2] = b.try_into().unwrap();
        let bits = match e {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        };
        half::f16::from_bits(bits)
    }
}

#[cfg(feature = "f16")]
impl EncodeToFile for half::f16 {
    const SIZE: usize = 2;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bits = self.to_bits();
        let bytes = match e {
            FileEndian::LittleEndian => bits.to_le_bytes(),
            FileEndian::BigEndian => bits.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

/// Raw buffer wrapper that prevents misuse
///
/// This wrapper enforces the invariant: raw bytes are opaque and can only
/// be accessed through decode/encode operations. No direct byte manipulation
/// is allowed outside the codec layer.
pub struct RawBuffer<'a> {
    bytes: &'a mut [u8],
    endian: FileEndian,
}

impl<'a> RawBuffer<'a> {
    /// Create a new raw buffer wrapper
    pub fn new(bytes: &'a mut [u8], endian: FileEndian) -> Self {
        Self { bytes, endian }
    }

    /// Read a typed value from the buffer at the given offset
    ///
    /// This is the ONLY way to get typed values from raw bytes.
    /// Endianness conversion is handled automatically.
    pub fn read<T: DecodeFromFile>(&self, offset: usize) -> T {
        T::decode(self.endian, &self.bytes[offset..offset + T::SIZE])
    }

    /// Write a typed value to the buffer at the given offset
    ///
    /// This is the ONLY way to write typed values to raw bytes.
    /// Endianness conversion is handled automatically.
    pub fn write<T: EncodeToFile>(&mut self, offset: usize, value: T) {
        value.encode(self.endian, &mut self.bytes[offset..offset + T::SIZE]);
    }

    /// Get the underlying bytes as a slice (read-only)
    ///
    /// This is provided for compatibility but should be used sparingly.
    /// Prefer using `read<T>()` for value-level access.
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Get mutable access to the underlying bytes
    ///
    /// This is provided for compatibility but should be used sparingly.
    /// Prefer using `write<T>()` for value-level access.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.bytes
    }

    /// Get the file endianness
    pub fn endian(&self) -> FileEndian {
        self.endian
    }
}

/// Optional file features
#[cfg(feature = "file")]
mod mrcfile;
#[cfg(test)]
#[cfg(feature = "file")]
#[path = "../test/mrcfile_test.rs"]
mod mrcfile_test;

#[cfg(feature = "mmap")]
pub use mrcfile::{MrcMmap, open_mmap};

#[cfg(feature = "file")]
pub use mrcfile::{MrcFile, open_file};

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error")]
    Io,
    #[error("Invalid MRC header")]
    InvalidHeader,
    #[error("Invalid MRC mode")]
    InvalidMode,
    #[error("Invalid dimensions")]
    InvalidDimensions,
    #[error("Type mismatch")]
    TypeMismatch,
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
}
