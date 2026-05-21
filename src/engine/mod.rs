//! Core encoding/decoding engine.
//!
//! This module implements the low-level I/O pipeline:
//!
//! ```text
//! Raw Bytes → Endian Normalization → Typed Values
//! ```
//!
//! Submodules provide:
//!
//! * [`block`] – volume geometry and voxel block types.
//! * [`codec`] – bidirectional endian codec for primitive types.
//! * [`convert`] – common type conversions (e.g. `i16` → `f32`).
//! * [`endian`] – endianness detection and the [`FileEndian`](endian::FileEndian) enum.
//! * [`stats`] – statistics computation for header validation.
//! * [`simd`] – SIMD-accelerated conversion kernels (optional `simd` feature).

pub mod block;
pub mod codec;
pub mod convert;
pub mod endian;
pub mod stats;

#[cfg(feature = "simd")]
pub mod simd;
