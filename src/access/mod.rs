//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `Volume`: 3D volume container
//! - `Slice2D`: 2D slice view into a volume (read-only)
//! - `Slice2DMut`: 2D slice view into a volume (mutable)
//! - `VolumeBuilder`: Fluent volume construction
//! - `VolumeData`: Dynamic dispatch for runtime mode handling

pub mod dynamic;
pub mod volume;

pub use dynamic::VolumeData;
pub use volume::{Slice2D, Slice2DMut, Volume, VolumeBuilder};
