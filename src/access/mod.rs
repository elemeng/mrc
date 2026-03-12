//! Data access patterns for MRC volumes
//!
//! This module provides:
//! - `VoxelAccess`: Trait for read-only voxel access
//! - `VoxelAccessMut`: Trait for mutable voxel access  
//! - `DataBlock`/`DataBlockMut`: Runtime-typed voxel access
//! - `Volume`: 3D volume container
//! - `VolumeData`: Dynamic dispatch for unknown types

pub mod traits;
pub mod block;
pub mod volume;
pub mod volume_trait;
pub mod dynamic;

pub use traits::{VoxelAccess, VoxelAccessMut};
pub use block::{DataBlock, DataBlockMut};
pub use volume::Volume;
pub use volume_trait::{VolumeMut, VolumeStats, VolumeExt, VolumeIter};
pub use dynamic::{VolumeData, DynVolume};
