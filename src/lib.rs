//! # MRC File Format Library
//!
//! This crate provides a safe, efficient, and endian-correct implementation for reading
//! and writing MRC (Medical Research Council) files, which are commonly used in
//! cryo-electron microscopy and structural biology.
//!
//! ## Endianness Guarantee
//!
//! **This crate guarantees that all raw byte ↔ typed value conversions are endian-correct.**
//!
//! Endianness handling is fully contained within the decode/encode layer and never leaks
//! into user code. The core invariant is:
//!
//! - **Raw bytes always represent file endian**
//! - **Typed values are always native endian**
//! - **The only place endian logic exists is at decode/encode boundaries**
//!
//! This design ensures:
//! - No full data swaps required
//! - Partial writes remain cheap
//! - Memory-mapped files remain viable
//! - Users never see endian concerns
//! - Developers cannot accidentally corrupt data
//!
//! ## Architecture
//!
//! The crate enforces endian safety through three layers:
//!
//! 1. **Codec Traits** (`DecodeFromFile`, `EncodeToFile`): The only place where endian
//!    conversion happens. All typed values must pass through these traits when converting
//!    to/from raw bytes.
//!
//! 2. **RawBuffer Wrapper**: Prevents misuse of raw bytes by only exposing value-level
//!    access through read/write methods that use the codec traits.
//!
//! 3. **Type-Safe Views**: `MrcView` and `MrcViewMut` provide safe access to file data
//!    with automatic endian conversion.
//!
//! ## Example
//!
//! ```ignore
//! use mrc::{MrcFile, Mode};
//!
//! // Open a file - endianness is handled automatically
//! let file = MrcFile::open("example.mrc")?;
//!
//! // Get a view of the data
//! let view = file.read_view()?;
//!
//! // Access data as native-endian f32 values
//! let data = view.data_as_f32()?;
//!
//! // Work with data in native endianness
//! for value in &data {
//!     println!("{}", value);
//! }
//! ```
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
    pub fn from_machst(machst: &[u8; 4]) -> Self {
        // Check first two bytes (bytes 213-214 in header)
        // 0x44 = 'D' in ASCII, indicates little-endian
        // 0x11 indicates big-endian
        if machst[0] == 0x44 && machst[1] == 0x44 {
            FileEndian::LittleEndian
        } else if machst[0] == 0x11 && machst[1] == 0x11 {
            FileEndian::BigEndian
        } else {
            // Default to little-endian for unknown values
            // (most common in practice)
            FileEndian::LittleEndian
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
