//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `VoxelAccess`: Trait for read-only voxel access
//! - `VoxelAccessMut`: Trait for mutable voxel access  
//! - `VolumeAccess`: Full 3D volume access with dimensions, strides, iteration
//! - `VolumeAccessMut`: Mutable 3D volume access
//! - `Volume`: N-dimensional volume container
//! - `VolumeData`: Dynamic dispatch for unknown types

pub mod dynamic;
pub mod traits;
pub mod volume;

pub use dynamic::{DynVolume, VolumeData};
pub use traits::{VoxelAccess, VoxelAccessMut, VolumeAccess, VolumeAccessMut, VolumeIter};
pub use volume::Volume;