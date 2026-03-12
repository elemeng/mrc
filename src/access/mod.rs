//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `Volume`: 3D volume container
//! - `Slice2D`: 2D slice view into a volume
//! - `VolumeBuilder`: Fluent volume construction
//! - `VolumeAccess`: Trait for statically typed 3D volume access
//! - `VolumeAccessMut`: Trait for mutable 3D volume access
//! - `VolumeData`: Dynamic dispatch for runtime mode handling

pub mod dynamic;
pub mod traits;
pub mod volume;

pub use dynamic::{DynVolume, VolumeData};
pub use traits::{VolumeAccess, VolumeAccessMut, VolumeIter};
pub use volume::{Image2D, Slice2D, Volume, VolumeBuilder};
