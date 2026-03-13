//! MRC header handling
//!
//! This module provides:
//! - `Header`: Validated header with semantic access (public API)
//! - `HeaderBuilder`: Fluent construction
//! - `ExtType`: Extended header type identifier

mod raw;
mod validated;

#[cfg(feature = "std")]
mod extended;

pub use validated::{Header, HeaderBuilder};

#[cfg(feature = "std")]
pub use extended::ExtType;

// RawHeader is internal - not exported
