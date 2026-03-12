//! Error types for MRC file operations

use crate::FileEndian;

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
    /// File endianness does not match native endianness, preventing zero-copy operations
    #[error("Wrong endianness: file is {file:?}, native is {native:?}")]
    WrongEndianness {
        file: FileEndian,
        native: FileEndian,
    },
    /// Data is not properly aligned for the requested type
    #[error("Misaligned data: required alignment {required}, got {actual}")]
    MisalignedData { required: usize, actual: usize },
    /// Buffer is too small for the requested operation
    #[error("Buffer too small: expected {expected} bytes, got {got}")]
    BufferTooSmall { expected: usize, got: usize },
    /// Index is out of bounds
    #[error("Index out of bounds: index {index}, length {length}")]
    IndexOutOfBounds { index: usize, length: usize },
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
}
