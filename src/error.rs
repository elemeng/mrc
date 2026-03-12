//! Error types for MRC file operations

use core::fmt;

#[cfg(feature = "std")]
extern crate alloc;

/// Error type for MRC operations
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Invalid MRC header
    InvalidHeader,
    /// Invalid MRC mode value
    InvalidMode,
    /// Invalid dimensions (negative or zero)
    InvalidDimensions,
    /// Invalid axis mapping
    InvalidAxisMap,
    /// Type mismatch for operation
    TypeMismatch,
    /// File endianness does not match native endianness
    WrongEndianness,
    /// Data is not properly aligned
    MisalignedData {
        required: usize,
        actual: usize,
    },
    /// Buffer is too small
    BufferTooSmall {
        expected: usize,
        got: usize,
    },
    /// Index out of bounds
    IndexOutOfBounds {
        index: usize,
        length: usize,
    },
    /// IO error
    #[cfg(feature = "std")]
    Io(alloc::string::String),
    /// Memory mapping error
    #[cfg(feature = "mmap")]
    Mmap,
    /// Feature not enabled
    FeatureDisabled {
        feature: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "Invalid MRC header"),
            Self::InvalidMode => write!(f, "Invalid MRC mode"),
            Self::InvalidDimensions => write!(f, "Invalid dimensions"),
            Self::InvalidAxisMap => write!(f, "Invalid axis mapping"),
            Self::TypeMismatch => write!(f, "Type mismatch"),
            Self::WrongEndianness => write!(f, "Wrong endianness"),
            Self::MisalignedData { required, actual } => {
                write!(f, "Misaligned data: required alignment {required}, got {actual}")
            }
            Self::BufferTooSmall { expected, got } => {
                write!(f, "Buffer too small: expected {expected} bytes, got {got}")
            }
            Self::IndexOutOfBounds { index, length } => {
                write!(f, "Index {index} out of bounds (length {length})")
            }
            #[cfg(feature = "std")]
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            #[cfg(feature = "mmap")]
            Self::Mmap => write!(f, "Memory mapping error"),
            Self::FeatureDisabled { feature } => {
                write!(f, "Feature '{feature}' is not enabled")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
