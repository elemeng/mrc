//! Error types for MRC I/O and validation.

/// The top-level error type for MRC I/O operations.
///
/// Most fallible functions in this crate return `Result<T, Error>`.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An underlying I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// The MRC header is malformed or fails basic validation.
    #[error("Invalid MRC header")]
    InvalidHeader,
    /// The MRC mode value is not supported by this crate.
    #[error("Unsupported mode")]
    UnsupportedMode,
    /// A requested read or write falls outside the volume bounds.
    #[error("Bounds error")]
    BoundsError,
    /// The voxel type does not match the file's mode.
    #[error("Type mismatch: expected {expected} bytes per voxel, got {actual} bytes")]
    TypeMismatch { expected: usize, actual: usize },
    /// The data vector length does not match the declared block shape.
    #[error("Invalid block shape: expected {expected} elements, got {actual}")]
    BlockShapeMismatch { expected: usize, actual: usize },

    /// The requested voxel type does not match the file's stored mode.
    #[error("Mode mismatch: file stores {file_mode:?}, requested {requested_mode:?}")]
    ModeMismatch {
        file_mode: crate::Mode,
        requested_mode: crate::Mode,
    },
    /// Detailed header validation failed.
    #[error("Invalid header: {0}")]
    InvalidHeaderDetailed(#[from] HeaderValidationError),
    /// Header statistics do not match the actual data.
    #[error(
        "Stats mismatch: header claims dmin={claimed_dmin}, dmax={claimed_dmax}, dmean={claimed_dmean}, rms={claimed_rms} but actual data has dmin={actual_dmin}, dmax={actual_dmax}, dmean={actual_dmean}, rms={actual_rms}"
    )]
    StatsMismatch {
        claimed_dmin: f32,
        claimed_dmax: f32,
        claimed_dmean: f32,
        claimed_rms: f32,
        actual_dmin: f32,
        actual_dmax: f32,
        actual_dmean: f32,
        actual_rms: f32,
    },
    /// Memory mapping failed (requires the `mmap` feature).
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
    /// The file size does not match the header's declared data size.
    #[error("File size mismatch: expected {expected} bytes, got {actual} bytes")]
    FileSizeMismatch { expected: usize, actual: usize },
    /// A volume-stack operation was requested on a file that is not a volume stack.
    #[error("Not a volume stack: ispg={ispg}, mz={mz} (expected ispg in 401-630 with mz > 0)")]
    NotAVolumeStack { ispg: i32, mz: i32 },
}

/// Errors that can occur during detailed header validation.
///
/// These are returned by [`Header::validate_detailed`](crate::Header::validate_detailed)
/// and surfaced through [`Error::InvalidHeaderDetailed`](crate::Error::InvalidHeaderDetailed).
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum HeaderValidationError {
    /// One or more volume dimensions are non-positive.
    #[error("Invalid dimensions: nx={nx}, ny={ny}, nz={nz} (must all be positive)")]
    InvalidDimensions { nx: i32, ny: i32, nz: i32 },
    /// The mode value is not recognized.
    #[error("Unsupported mode: {0}")]
    UnsupportedMode(i32),
    /// The MAP identifier field is not valid.
    #[error("Invalid MAP field: expected 'MAP ', got {0:?}")]
    InvalidMap([u8; 4]),
    /// The space group number is outside the valid ranges.
    #[error("Invalid ISPG: {0} (expected 0, 1-230, or 400-630)")]
    InvalidIspg(i32),
    /// The axis mapping is not a permutation of (1, 2, 3).
    #[error(
        "Invalid axis mapping: mapc={mapc}, mapr={mapr}, maps={maps} (must be a permutation of 1,2,3)"
    )]
    InvalidAxisMapping { mapc: i32, mapr: i32, maps: i32 },
    /// The extended header size is negative.
    #[error("Invalid nsymbt: {0} (must be non-negative)")]
    InvalidNsymbt(i32),
    /// The label count is outside the range 0–10.
    #[error("Invalid nlabl: {0} (must be between 0 and 10)")]
    InvalidNlabl(i32),
    /// The NVERSION value is not 20140 or 20141.
    #[error("Invalid nversion: {0} (expected 20140 or 20141)")]
    InvalidNversion(i32),
    /// Volume stack consistency check failed (`nz` must be divisible by `mz`).
    #[error(
        "Invalid volume stack: nz={nz} is not divisible by mz={mz} (required when ispg={ispg} indicates a volume stack)"
    )]
    InvalidVolumeStack { nz: i32, mz: i32, ispg: i32 },
    /// One or more sampling values are non-positive.
    #[error("Invalid sampling: mx={mx}, my={my}, mz={mz} (must all be positive)")]
    InvalidSampling { mx: i32, my: i32, mz: i32 },
    /// The declared label count does not match the actual non-empty labels.
    #[error("Label count mismatch: nlabl={nlabl} but {actual} non-empty labels found")]
    LabelCountMismatch { nlabl: i32, actual: i32 },
    /// A gap was found in the label sequence.
    #[error("Empty label at index {index} before all filled labels")]
    EmptyLabelBeforeFilled { index: i32 },
}
