//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `Volume`: 3D volume container
//! - `Slice2D` / `Slice2DMut`: 2D plane extracted from 3D volume at Z position
//! - `VolumeBuilder`: Fluent volume construction
//! - `VolumeData`: Dynamic dispatch for runtime mode handling
//!
//! # Terminology: "Slice"
//!
//! | Term | Meaning |
//! |------|---------|
//! | `Slice2D`, `Slice2DMut` | Geometric 2D plane in 3D space |
//! | `Volume::as_slice()` | Rust `&[T]` data view |
//!
//! See [`volume`](crate::access::volume) module for details.

pub mod dynamic;
pub mod volume;

pub use dynamic::VolumeData;
pub use volume::{Slice2D, Slice2DMut, Volume, VolumeBuilder};
