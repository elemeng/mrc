//! Extended header types and handling
//!
//! MRC extended headers come beyond the 1024-byte main header.
//! They contain application-specific metadata and vary by type.

extern crate alloc;

use alloc::vec::Vec;

/// Extended header type identifier
///
/// Common values from MRC2014 spec and implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
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

impl ExtType {
    /// Create from 4-byte EXTTYP field
    pub fn from_bytes(bytes: &[u8; 4]) -> Self {
        // Check for common patterns
        let s = core::str::from_utf8(bytes).unwrap_or("");
        match s {
            "CCP4" => Self::Ccp4,
            "MRCO" => Self::Mrco,
            "SERI" => Self::Seri,
            "AGAR" => Self::Agar,
            "FEI1" => Self::Fei1,
            "FEI2" => Self::Fei2,
            "HDF5" => Self::Hdf5,
            _ => Self::Unknown,
        }
    }
    
    /// Get the 4-byte identifier
    pub fn as_bytes(&self) -> [u8; 4] {
        match self {
            Self::Ccp4 => *b"CCP4",
            Self::Mrco => *b"MRCO",
            Self::Seri => *b"SERI",
            Self::Agar => *b"AGAR",
            Self::Fei1 => *b"FEI1",
            Self::Fei2 => *b"FEI2",
            Self::Hdf5 => *b"HDF5",
            Self::Unknown => [0, 0, 0, 0],
        }
    }
}


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