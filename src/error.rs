//! Error types for MRC operations.

/// The top-level error type for MRC I/O operations.
///
/// Most fallible functions in this crate return `Result<T, Error>`.
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
    #[error("Stats mismatch: header claims dmin={claimed_dmin}, dmax={claimed_dmax}, dmean={claimed_dmean}, rms={claimed_rms} but actual data has dmin={actual_dmin}, dmax={actual_dmax}, dmean={actual_dmean}, rms={actual_rms}")]
    StatsMismatch {
        claimed_dmin: f32, claimed_dmax: f32, claimed_dmean: f32, claimed_rms: f32,
        actual_dmin: f32, actual_dmax: f32, actual_dmean: f32, actual_rms: f32,
    },
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
    #[error("File size mismatch: expected {expected} bytes, got {actual} bytes")]
    FileSizeMismatch { expected: usize, actual: usize },
}

/// Errors that can occur during detailed header validation.
///
/// These are returned by [`Header::validate_detailed`](crate::Header::validate_detailed)
/// and surfaced through [`Error::InvalidHeaderDetailed`](crate::Error::InvalidHeaderDetailed).
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
    #[error("Invalid nversion: {0} (expected 20140 or 20141)")]
    InvalidNversion(i32),
    #[error("Invalid volume stack: nz={nz} is not divisible by mz={mz} (required when ispg={ispg} indicates a volume stack)")]
    InvalidVolumeStack { nz: i32, mz: i32, ispg: i32 },
    #[error("Invalid sampling: mx={mx}, my={my}, mz={mz} (must all be positive)")]
    InvalidSampling { mx: i32, my: i32, mz: i32 },
    #[error("Label count mismatch: nlabl={nlabl} but {actual} non-empty labels found")]
    LabelCountMismatch { nlabl: i32, actual: i32 },
    #[error("Empty label at index {index} before all filled labels")]
    EmptyLabelBeforeFilled { index: i32 },
}

