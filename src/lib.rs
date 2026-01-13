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

#![no_std]
#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "f16")]
extern crate half;

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
                    machst[0], machst[1], machst[2], machst[3]
                );
            }
        }

        endian
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

// Optional file features
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

// Error type

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
