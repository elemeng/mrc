//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `VoxelAccess`: Trait for read-only voxel access
//! - `VoxelAccessMut`: Trait for mutable voxel access  
//! - `Volume`: N-dimensional volume container (replaces DataBlock)
//! - `VolumeData`: Dynamic dispatch for unknown types

pub mod dynamic;
pub mod traits;
pub mod volume;
pub mod volume_trait;

pub use dynamic::{DynVolume, VolumeData};
pub use traits::{VoxelAccess, VoxelAccessMut};
pub use volume::Volume;
pub use volume_trait::{VolumeExt, VolumeIter, VolumeMut, VolumeStats};
