//! Error types for MRC operations

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid MRC header")]
    InvalidHeader,
    #[error("Unsupported mode")]
    UnsupportedMode,
    #[error("Bounds error")]
    BoundsError,
    #[error("Type mismatch: expected {expected} bytes per voxel, got {actual} bytes")]
    TypeMismatch { expected: usize, actual: usize },
    #[error("Invalid block shape: expected {expected} elements, got {actual}")]
    BlockShapeMismatch { expected: usize, actual: usize },

    #[error("Mode mismatch: file stores {file_mode:?}, requested {requested_mode:?}")]
    ModeMismatch { file_mode: crate::Mode, requested_mode: crate::Mode },
    #[error("Invalid header: {0}")]
    InvalidHeaderDetailed(#[from] HeaderValidationError),
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
}

/// Errors that can occur during header validation.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum HeaderValidationError {
    #[error("Invalid dimensions: nx={nx}, ny={ny}, nz={nz} (must all be positive)")]
    InvalidDimensions { nx: i32, ny: i32, nz: i32 },
    #[error("Unsupported mode: {0}")]
    UnsupportedMode(i32),
    #[error("Invalid MAP field: expected 'MAP ', got {0:?}")]
    InvalidMap([u8; 4]),
    #[error("Invalid ISPG: {0} (expected 0, 1-230, or 400-630)")]
    InvalidIspg(i32),
    #[error("Invalid axis mapping: mapc={mapc}, mapr={mapr}, maps={maps} (must be a permutation of 1,2,3)")]
    InvalidAxisMapping { mapc: i32, mapr: i32, maps: i32 },
    #[error("Invalid nsymbt: {0} (must be non-negative)")]
    InvalidNsymbt(i32),
    #[error("Invalid nlabl: {0} (must be between 0 and 10)")]
    InvalidNlabl(i32),
}


