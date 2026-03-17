//! Error types for MRC operations

use alloc::string::String;

/// Error type
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(String),
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
