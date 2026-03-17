//! Unified encoding/decoding engine
//!
//! This module implements the pipeline architecture:
//! ```text
//! Raw Bytes → Endian Normalization → Typed Values → Type Conversion
//! ```
//!
//! With zero-copy fast paths whenever possible.

pub mod block;
pub mod codec;
pub mod convert;
pub mod endian;
pub mod pipeline;
