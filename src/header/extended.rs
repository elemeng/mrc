//! Extended header types and handling
//!
//! MRC extended headers come beyond the 1024-byte main header.
//! They contain application-specific metadata and vary by type.
//!
//! This module provides minimal parsing - just the type code and raw bytes.
//! Future versions may add specific parsers for different formats.

extern crate alloc;

use alloc::vec::Vec;

/// Extended header type identifier (4-byte EXTTYP field)
///
/// Common values from MRC2014 spec and implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtType {
    /// CCP4 format (classic)
    Ccp4,
    /// MRCO format (MRC original)
    Mrco,
    /// SerialEM format (FEI)
    Seri,
    /// Agard format
    Agar,
    /// FEI format version 1
    Fei1,
    /// FEI format version 2
    Fei2,
    /// HDF5 format
    Hdf5,
    /// Unknown/unrecognized format
    #[default]
    Unknown,
}

// Macro to define ExtType variants with their string representations
macro_rules! ext_type_variants {
    ($(($variant:ident, $str:literal)),* $(,)?) => {
        impl ExtType {
            /// Create from 4-byte EXTTYP field
            pub fn from_bytes(bytes: &[u8; 4]) -> Self {
                let s = core::str::from_utf8(bytes).unwrap_or("");
                match s {
                    $($str => Self::$variant,)*
                    _ => Self::Unknown,
                }
            }

            /// Get the 4-byte identifier
            pub fn as_bytes(&self) -> [u8; 4] {
                match self {
                    $(Self::$variant => {
                        let mut arr = [0u8; 4];
                        arr.copy_from_slice($str.as_bytes());
                        arr
                    },)*
                    Self::Unknown => [0, 0, 0, 0],
                }
            }
        }
    };
}

ext_type_variants!(
    (Ccp4, "CCP4"),
    (Mrco, "MRCO"),
    (Seri, "SERI"),
    (Agar, "AGAR"),
    (Fei1, "FEI1"),
    (Fei2, "FEI2"),
    (Hdf5, "HDF5"),
);

impl ExtType {
    /// Get the type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ccp4 => "CCP4",
            Self::Mrco => "MRCO",
            Self::Seri => "SERI",
            Self::Agar => "AGAR",
            Self::Fei1 => "FEI1",
            Self::Fei2 => "FEI2",
            Self::Hdf5 => "HDF5",
            Self::Unknown => "",
        }
    }
}

impl core::fmt::Display for ExtType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Extended header - minimal wrapper for raw bytes
///
/// Provides access to the type code and raw bytes only.
/// Parsing is left to users or future v2 specific parsers.
///
/// # Example
/// ```ignore
/// let ext = reader.ext_header();
/// match ext.code() {
///     ExtType::Seri => {
///         // Parse SerialEM format from ext.raw_bytes()
///     }
///     _ => {}
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ExtendedHeader {
    /// Type identifier (from EXTTYP field)
    code: ExtType,
    /// Raw bytes of the extended header (from NSYMBT bytes after header)
    data: Vec<u8>,
}

impl ExtendedHeader {
    /// Create a new extended header
    pub fn new(code: ExtType, data: Vec<u8>) -> Self {
        Self { code, data }
    }

    /// Create from EXTTYP bytes and raw data
    pub fn from_bytes(exttyp: &[u8; 4], data: Vec<u8>) -> Self {
        let code = ExtType::from_bytes(exttyp);
        Self { code, data }
    }

    /// Create an empty extended header
    pub fn empty() -> Self {
        Self {
            code: ExtType::Unknown,
            data: Vec::new(),
        }
    }

    /// Get the type code
    pub fn code(&self) -> ExtType {
        self.code
    }

    /// Get the raw bytes
    pub fn raw_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the byte range (offset, length) for this header in the file
    ///
    /// The extended header starts at offset 1024 (after main header).
    pub fn bytes_range(&self) -> (usize, usize) {
        (1024, self.data.len())
    }

    /// Get the size in bytes
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the EXTTYP as bytes
    pub fn exttyp_bytes(&self) -> [u8; 4] {
        self.code.as_bytes()
    }
}
