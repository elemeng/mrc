//! MRC header handling
//!
//! This module provides two header types:
//! - `RawHeader`: Direct binary mapping of the 1024-byte MRC header
//! - `Header`: Validated header with semantic access
//! - `ExtendedHeader`: Extended header data

#[cfg(feature = "std")]
mod extended;
mod raw;
mod validated;

pub use raw::RawHeader;
pub use validated::{Header, HeaderBuilder};

#[cfg(feature = "std")]
pub use extended::{ExtType, ExtendedHeader};
