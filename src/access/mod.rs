//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `Volume`: 3D volume container
//! - `Slice2D`: 2D slice view into a volume (read-only)
//! - `Slice2DMut`: 2D slice view into a volume (mutable)
//! - `VolumeBuilder`: Fluent volume construction
//! - `VolumeAccess`: Trait for statically typed 3D volume access
//! - `VolumeAccessMut`: Trait for mutable 3D volume access
//! - `VolumeData`: Dynamic dispatch for runtime mode handling

pub mod dynamic;
pub mod traits;
pub mod volume;

pub use dynamic::VolumeData;
pub use traits::{VolumeAccess, VolumeAccessMut, VolumeIter};
pub use volume::{Slice2D, Slice2DMut, Volume, VolumeBuilder};
