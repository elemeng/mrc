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
