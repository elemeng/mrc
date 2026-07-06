//! Error types for MRC I/O and validation.
//!
//! The [`Error`] enum is the single error type returned by all fallible
//! operations in this crate. It covers I/O failures, header issues, type
//! mismatches, bounds errors, compression problems, and statistics mismatches.
//!
//! [`HeaderValidationError`] provides fine-grained diagnostics for header
//! problems — dimensions, axis mapping, space group, labels, NVERSION, etc.
//!
//! # Example — matching specific errors
//!
//! ```rust
//! use mrc::Error;
//!
//! fn check(err: &Error) -> &'static str {
//!     match err {
//!         Error::Io(_) => "I/O problem",
//!         Error::InvalidHeader => "bad header",
//!         Error::ModeMismatch { .. } => "wrong voxel type for this file",
//!         Error::BoundsError { .. } => "access outside volume",
//!         Error::FileSizeMismatch { .. } => "file truncated or has trailing data",
//!         _ => "other",
//!     }
//! }
//! assert_eq!(check(&Error::BoundsError { offset: None, shape: None, volume: None }), "access outside volume");
//! ```

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The top-level error type for MRC I/O operations.
///
/// Most fallible functions in this crate return `Result<T, Error>`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An underlying I/O operation failed.
    #[error("IO error: {0}")]
    #[cfg_attr(feature = "serde", serde(skip))]
    Io(#[from] std::io::Error),
    /// The MRC header is malformed or fails basic validation.
    #[error("Invalid MRC header")]
    InvalidHeader,
    /// The MRC mode value is not supported by this crate.
    #[error("Unsupported mode")]
    UnsupportedMode,
    /// A requested read or write falls outside the volume bounds.
    ///
    /// The optional fields provide context about which block was requested
    /// and what the volume dimensions were.
    #[error(
        "Bounds error at offset ({ox},{oy},{oz}) shape ({sx},{sy},{sz}) of volume ({vx},{vy},{vz})",
        ox = .offset.map_or(0, |o| o[0]),
        oy = .offset.map_or(0, |o| o[1]),
        oz = .offset.map_or(0, |o| o[2]),
        sx = .shape.map_or(0, |s| s[0]),
        sy = .shape.map_or(0, |s| s[1]),
        sz = .shape.map_or(0, |s| s[2]),
        vx = .volume.map_or(0, |v| v[0]),
        vy = .volume.map_or(0, |v| v[1]),
        vz = .volume.map_or(0, |v| v[2]),
    )]
    BoundsError {
        /// Offset of the requested block `[x, y, z]`.
        offset: Option<[usize; 3]>,
        /// Shape of the requested block `[sx, sy, sz]`.
        shape: Option<[usize; 3]>,
        /// Dimensions of the volume `[nx, ny, nz]`.
        volume: Option<[usize; 3]>,
    },
    /// The voxel type does not match the file's mode.
    #[error("Type mismatch: expected {expected} bytes per voxel, got {actual} bytes")]
    TypeMismatch {
        /// Number of bytes per voxel expected by the decoder.
        expected: usize,
        /// Number of bytes provided.
        actual: usize,
    },
    /// The data vector length does not match the declared block shape.
    #[error("Invalid block shape: expected {expected} elements, got {actual}")]
    BlockShapeMismatch {
        /// Number of elements expected from the shape.
        expected: usize,
        /// Number of elements actually provided.
        actual: usize,
    },

    /// The requested voxel type does not match the file's stored mode.
    #[error("Mode mismatch: file stores {file_mode:?}, requested {requested_mode:?}{}",
        match .offset {
            Some(o) => format!(" at offset ({},{},{})", o[0], o[1], o[2]),
            None => String::new(),
        }
    )]
    ModeMismatch {
        /// The mode stored in the file.
        file_mode: crate::Mode,
        /// The mode requested by the caller.
        requested_mode: crate::Mode,
        /// Offset where the mismatch was detected `[x, y, z]`.
        offset: Option<[usize; 3]>,
    },
    /// Detailed header validation failed.
    #[error("Invalid header: {0}")]
    InvalidHeaderDetailed(#[from] HeaderValidationError),
    /// Header statistics do not match the actual data.
    #[error(
        "Stats mismatch: header claims dmin={claimed_dmin}, dmax={claimed_dmax}, dmean={claimed_dmean}, rms={claimed_rms} but actual data has dmin={actual_dmin}, dmax={actual_dmax}, dmean={actual_dmean}, rms={actual_rms}"
    )]
    StatsMismatch {
        /// Minimum density value claimed in the header.
        claimed_dmin: f32,
        /// Maximum density value claimed in the header.
        claimed_dmax: f32,
        /// Mean density value claimed in the header.
        claimed_dmean: f32,
        /// RMS deviation claimed in the header.
        claimed_rms: f32,
        /// Actual minimum density computed from the data.
        actual_dmin: f32,
        /// Actual maximum density computed from the data.
        actual_dmax: f32,
        /// Actual mean density computed from the data.
        actual_dmean: f32,
        /// Actual RMS deviation computed from the data.
        actual_rms: f32,
    },
    /// Memory mapping failed (requires the `mmap` feature).
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
    /// The file size does not match the header's declared data size.
    #[error("File size mismatch: expected {expected} bytes, got {actual} bytes")]
    FileSizeMismatch {
        /// Expected file size in bytes (header + extended header + data).
        expected: usize,
        /// Actual file size in bytes.
        actual: usize,
    },
    /// A volume-stack operation was requested on a file that is not a volume stack.
    #[error("Not a volume stack: ispg={ispg}, mz={mz} (expected ispg in 401-630 with mz > 0)")]
    NotAVolumeStack {
        /// The ISPG (space group) value from the header.
        ispg: i32,
        /// The MZ (sampling along Z) value from the header.
        mz: i32,
    },
}

impl Error {
    /// Create a bounds error without detailed context.
    ///
    /// Use this in cold error paths where the offset/shape/volume are not
    /// immediately available.  Prefer [`BoundsError`](Self::BoundsError) with
    /// populated fields at validation boundaries.
    #[cold]
    pub(crate) fn bounds_err() -> Self {
        Self::BoundsError {
            offset: None,
            shape: None,
            volume: None,
        }
    }
}

/// Errors that can occur during detailed header validation.
///
/// These are returned by [`Header::validate_detailed`](crate::Header::validate_detailed)
/// and surfaced through [`Error::InvalidHeaderDetailed`](crate::Error::InvalidHeaderDetailed).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum HeaderValidationError {
    /// One or more volume dimensions are non-positive.
    #[error("Invalid dimensions: nx={nx}, ny={ny}, nz={nz} (must all be positive)")]
    InvalidDimensions {
        /// Number of columns (X axis).
        nx: i32,
        /// Number of rows (Y axis).
        ny: i32,
        /// Number of sections (Z axis).
        nz: i32,
    },
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
    InvalidAxisMapping {
        /// Column axis index.
        mapc: i32,
        /// Row axis index.
        mapr: i32,
        /// Section axis index.
        maps: i32,
    },
    /// The extended header size is negative.
    #[error("Invalid nsymbt: {0} (must be non-negative)")]
    InvalidNsymbt(i32),
    /// The label count is outside the range 0–10.
    #[error("Invalid nlabl: {0} (must be between 0 and 10)")]
    InvalidNlabl(i32),
    /// The NVERSION value is not 0, 20140, or 20141.
    #[error("Invalid nversion: {0} (expected 0, 20140, or 20141)")]
    InvalidNversion(i32),
    /// Volume stack consistency check failed.
    #[error(
        "Invalid volume stack: nz={nz} is not divisible by mz={mz} (required when ispg={ispg} indicates a volume stack)"
    )]
    InvalidVolumeStack {
        /// Total number of sections.
        nz: i32,
        /// Sections per sub-volume.
        mz: i32,
        /// Space group number.
        ispg: i32,
    },
    /// One or more sampling values are non-positive.
    #[error("Invalid sampling: mx={mx}, my={my}, mz={mz} (must all be positive)")]
    InvalidSampling {
        /// Sampling along X.
        mx: i32,
        /// Sampling along Y.
        my: i32,
        /// Sampling along Z.
        mz: i32,
    },
    /// The declared label count does not match the actual non-empty labels.
    #[error("Label count mismatch: nlabl={nlabl} but {actual} non-empty labels found")]
    LabelCountMismatch {
        /// Declared label count in the header.
        nlabl: i32,
        /// Actual number of non-empty labels.
        actual: i32,
    },
    /// A gap was found in the label sequence.
    #[error("Empty label at index {index} before all filled labels")]
    EmptyLabelBeforeFilled {
        /// Index of the empty label slot.
        index: i32,
    },
}
