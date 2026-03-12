//! Error types for MRC file operations

use core::fmt;

#[cfg(feature = "std")]
extern crate alloc;

/// Error type for MRC operations
#[derive(Debug)]
pub enum Error {
    /// Invalid MRC header
    InvalidHeader,
    /// Invalid MRC mode value
    InvalidMode,
    /// Invalid dimensions (negative or zero)
    InvalidDimensions,
    /// Invalid axis mapping
    InvalidAxisMap,
    /// Data is not contiguous in memory
    NonContiguous,
    /// Type mismatch for operation
    TypeMismatch,
    /// Endianness mismatch between file and native
    EndiannessMismatch { detected: bool },
    /// Data is not properly aligned
    MisalignedData { required: usize, actual: usize },
    /// Buffer is too small
    BufferTooSmall { expected: usize, got: usize },
    /// Index out of bounds
    IndexOutOfBounds { index: usize, length: usize },
    /// IO error
    #[cfg(feature = "std")]
    Io(alloc::boxed::Box<dyn std::error::Error + Send + Sync>),
    /// Memory mapping error
    #[cfg(feature = "mmap")]
    Mmap,
    /// Feature not enabled
    FeatureDisabled { feature: &'static str },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "Invalid MRC header"),
            Self::InvalidMode => write!(f, "Invalid MRC mode"),
            Self::InvalidDimensions => write!(f, "Invalid dimensions"),
            Self::InvalidAxisMap => write!(f, "Invalid axis mapping"),
            Self::NonContiguous => write!(f, "Data is not contiguous in memory"),
            Self::TypeMismatch => write!(f, "Type mismatch"),
            Self::EndiannessMismatch { detected } => {
                write!(f, "Endianness mismatch (detected: {detected})")
            }
            Self::MisalignedData { required, actual } => {
                write!(
                    f,
                    "Misaligned data: required alignment {required}, got {actual}"
                )
            }
            Self::BufferTooSmall { expected, got } => {
                write!(f, "Buffer too small: expected {expected} bytes, got {got}")
            }
            Self::IndexOutOfBounds { index, length } => {
                write!(f, "Index {index} out of bounds (length {length})")
            }
            #[cfg(feature = "std")]
            Self::Io(err) => write!(f, "IO error: {err}"),
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

// Macro for simple Copy-able error variants (no fields or Copy fields)
macro_rules! match_simple_variants {
    ($self:expr, $other:expr, $variant:ident) => {
        match ($self, $other) {
            (Self::$variant, Self::$variant) => true,
            _ => false,
        }
    };
}

// Macro for error variants with named fields - uses helper to chain comparisons
macro_rules! match_field_variants {
    ($self:expr, $other:expr, $variant:ident, { $field1:ident }) => {
        match ($self, $other) {
            (Self::$variant { $field1: a }, Self::$variant { $field1: b }) => a == b,
            _ => false,
        }
    };
    ($self:expr, $other:expr, $variant:ident, { $field1:ident, $field2:ident }) => {
        match ($self, $other) {
            (
                Self::$variant {
                    $field1: a1,
                    $field2: a2,
                },
                Self::$variant {
                    $field1: b1,
                    $field2: b2,
                },
            ) => a1 == b1 && a2 == b2,
            _ => false,
        }
    };
}

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidHeader => Self::InvalidHeader,
            Self::InvalidMode => Self::InvalidMode,
            Self::InvalidDimensions => Self::InvalidDimensions,
            Self::InvalidAxisMap => Self::InvalidAxisMap,
            Self::NonContiguous => Self::NonContiguous,
            Self::TypeMismatch => Self::TypeMismatch,
            Self::EndiannessMismatch { detected } => Self::EndiannessMismatch {
                detected: *detected,
            },
            Self::MisalignedData { required, actual } => Self::MisalignedData {
                required: *required,
                actual: *actual,
            },
            Self::BufferTooSmall { expected, got } => Self::BufferTooSmall {
                expected: *expected,
                got: *got,
            },
            Self::IndexOutOfBounds { index, length } => Self::IndexOutOfBounds {
                index: *index,
                length: *length,
            },
            Self::FeatureDisabled { feature } => Self::FeatureDisabled { feature },
            #[cfg(feature = "std")]
            Self::Io(_) => Self::Io(alloc::boxed::Box::new(std::io::Error::other(
                "IO error (cloned)",
            ))),
            #[cfg(feature = "mmap")]
            Self::Mmap => Self::Mmap,
        }
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match_simple_variants!(self, other, InvalidHeader)
            || match_simple_variants!(self, other, InvalidMode)
            || match_simple_variants!(self, other, InvalidDimensions)
            || match_simple_variants!(self, other, InvalidAxisMap)
            || match_simple_variants!(self, other, NonContiguous)
            || match_simple_variants!(self, other, TypeMismatch)
            || match_field_variants!(self, other, EndiannessMismatch, { detected })
            || match_field_variants!(self, other, MisalignedData, { required, actual })
            || match_field_variants!(self, other, BufferTooSmall, { expected, got })
            || match_field_variants!(self, other, IndexOutOfBounds, { index, length })
            || match_field_variants!(self, other, FeatureDisabled, { feature })
            || {
                #[cfg(feature = "std")]
                if let (Self::Io(_), Self::Io(_)) = (self, other) {
                    return true;
                }
                #[cfg(feature = "mmap")]
                if let (Self::Mmap, Self::Mmap) = (self, other) {
                    return true;
                }
                false
            }
    }
}

impl Eq for Error {}

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(alloc::boxed::Box::new(err))
    }
}

/// Check if index is within bounds, returning error if not
///
/// Helper function to reduce boilerplate bounds checking.
#[inline]
pub fn check_bounds(index: usize, length: usize) -> Result<(), Error> {
    if index >= length {
        return Err(Error::IndexOutOfBounds { index, length });
    }
    Ok(())
}
