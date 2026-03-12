//! MRC header handling
//!
//! This module provides:
//! - `Header`: Validated header with semantic access (public API)
//! - `HeaderBuilder`: Fluent construction
//! - `ExtendedHeader`: Extended header data

mod raw;
mod validated;

#[cfg(feature = "std")]
mod extended;

pub use validated::{Header, HeaderBuilder};

#[cfg(feature = "std")]
pub use extended::{ExtType, ExtendedHeader};

// RawHeader is internal - not exported
