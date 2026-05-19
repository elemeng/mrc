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
    #[error("Conversion error: {0}")]
    Conversion(ConversionError),
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
}

/// Result of checking whether a slice of values fits within a target type's range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeCheck {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub values_out_of_range: usize,
    pub total_values: usize,
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

/// Errors that can occur during type conversion operations.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ConversionError {
    #[error("Value out of range: input range [{min}, {max}] exceeds target range [{target_min}, {target_max}]")]
    OutOfRange {
        min: f64,
        max: f64,
        target_min: f64,
        target_max: f64,
    },
    #[error("NaN value encountered during conversion")]
    NaNValue,
    #[error("Infinity value encountered during conversion")]
    InfinityValue,
    #[error("Missing complex-to-real conversion strategy")]
    MissingComplexStrategy,
    #[error("Mode 3 is obsolete and should not be used for writing")]
    ObsoleteMode3,
    #[error("Packing data into 4-bit mode is not supported (data would be lost)")]
    PackingInto4Bit,
}
