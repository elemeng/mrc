//! Core types for MRC file handling
//!
//! This module provides fundamental types that work in no_std environments:
//! - Error types
//! - Mode enumeration
//! - Axis mapping

pub mod axis;
pub mod error;
pub mod mode;

pub use axis::AxisMap;
pub use error::Error;
pub use mode::{InvalidMode, Mode};

// Internal helpers
pub(crate) use error::check_bounds;
