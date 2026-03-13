//! Extended header type identifier
//!
//! MRC extended headers come beyond the 1024-byte main header.
//! They contain application-specific metadata and vary by type.
//!
//! This module only provides the EXTTYP identifier. Raw bytes are
//! accessible via `MrcReader::ext_header()`. Parsing is left to users.

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
