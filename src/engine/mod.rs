//! Unified encoding/decoding engine
//!
//! This module implements the core I/O layer:
//! ```text
//! Raw Bytes → Endian Normalization → Typed Values
//! ```
//!
//! Type conversion is the caller's responsibility.

pub mod block;
pub mod codec;
pub mod convert;
pub mod endian;
pub mod stats;

#[cfg(feature = "simd")]
pub mod simd;
