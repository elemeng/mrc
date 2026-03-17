//! Error types for MRC operations

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error")]
    Io,
    #[error("Invalid MRC header")]
    InvalidHeader,
    #[error("Unsupported mode")]
    UnsupportedMode,
    #[error("Bounds error")]
    BoundsError,
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
}
