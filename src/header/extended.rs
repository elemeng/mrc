//! Extended header types and handling
//!
//! MRC extended headers come beyond the 1024-byte main header.
//! They contain application-specific metadata and vary by type.

extern crate alloc;

use alloc::vec::Vec;

/// Extended header type identifier
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

/// Extended header wrapper
///
/// Provides access to extended header data which EXTTYP.
#[derive(Debug, Clone)]
pub struct ExtendedHeader {
    /// Extended header type identifier
    pub ext_type: ExtType,
    /// Raw bytes of the extended header
    pub data: Vec<u8>,
}

impl ExtendedHeader {
    /// Create a new extended header
    pub fn new(ext_type: ExtType, data: Vec<u8>) -> Self {
        Self { ext_type, data }
    }

    /// Create from raw bytes with EXTTYP identification
    pub fn from_bytes(exttyp: &[u8; 4], data: Vec<u8>) -> Self {
        let ext_type = ExtType::from_bytes(exttyp);
        Self { ext_type, data }
    }

    /// Create an empty extended header
    pub fn empty() -> Self {
        Self {
            ext_type: ExtType::Unknown,
            data: Vec::new(),
        }
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
        self.ext_type.as_bytes()
    }
}
