//! Dynamic dispatch for runtime mode handling

use super::Volume;
use crate::core::{Error, Mode};
use crate::header::Header;
use crate::voxel::{Encoding, Float32Complex, Int16Complex, Packed4Bit, Voxel};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Macro to generate typed downcast methods for VolumeData
macro_rules! impl_downcast {
    ($method:ident, $type:ty, $doc:expr) => {
        #[doc = $doc]
        pub fn $method(&self) -> Option<&Volume<$type, Vec<u8>>> {
            self.downcast_ref::<$type>()
        }
    };
}

/// Trait for dynamic volume operations
pub trait DynVolume {
    fn header(&self) -> &Header;
    fn mode(&self) -> Mode;
    fn dimensions(&self) -> (usize, usize, usize);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn as_any(&self) -> &dyn core::any::Any;
}

impl<T: Voxel + Encoding> DynVolume for Volume<T, Vec<u8>> {
    fn header(&self) -> &Header {
        super::volume_trait::Volume::header(self)
    }
    fn mode(&self) -> Mode {
        self.header().mode()
    }
    fn dimensions(&self) -> (usize, usize, usize) {
        super::volume_trait::Volume::dimensions(self)
    }
    fn len(&self) -> usize {
        super::Volume::len(self)
    }
    fn is_empty(&self) -> bool {
        super::Volume::is_empty(self)
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// Runtime-typed volume data
pub struct VolumeData(Box<dyn DynVolume>);

impl core::fmt::Debug for VolumeData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VolumeData")
            .field("mode", &self.mode())
            .field("dimensions", &self.dimensions())
            .finish()
    }
}

impl VolumeData {
    /// Create VolumeData from raw bytes and header
    pub fn from_bytes(header: Header, data: Vec<u8>) -> Result<Self, Error> {
        let boxed: Box<dyn DynVolume> = match header.mode() {
            Mode::Int8 => Box::new(Volume::<i8, Vec<u8>>::new(header, data)?),
            Mode::Int16 => Box::new(Volume::<i16, Vec<u8>>::new(header, data)?),
            Mode::Float32 => Box::new(Volume::<f32, Vec<u8>>::new(header, data)?),
            Mode::Int16Complex => Box::new(Volume::<Int16Complex, Vec<u8>>::new(header, data)?),
            Mode::Float32Complex => Box::new(Volume::<Float32Complex, Vec<u8>>::new(header, data)?),
            Mode::Uint16 => Box::new(Volume::<u16, Vec<u8>>::new(header, data)?),
            #[cfg(feature = "f16")]
            Mode::Float16 => Box::new(Volume::<half::f16, Vec<u8>>::new(header, data)?),
            Mode::Packed4Bit => Box::new(Volume::<Packed4Bit, Vec<u8>>::new(header, data)?),
        };
        Ok(Self(boxed))
    }

    /// Get the mode
    pub fn mode(&self) -> Mode {
        self.0.mode()
    }

    /// Get the header
    pub fn header(&self) -> &Header {
        self.0.header()
    }

    /// Get dimensions
    pub fn dimensions(&self) -> (usize, usize, usize) {
        self.0.dimensions()
    }

    /// Total voxels
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Downcast to typed volume
    pub fn downcast_ref<T: Voxel + Encoding>(&self) -> Option<&Volume<T, Vec<u8>>> {
        self.0.as_any().downcast_ref::<Volume<T, Vec<u8>>>()
    }

    // Generate typed downcast methods using macro
    impl_downcast!(as_f32, f32, "Try to get as f32 volume");
    impl_downcast!(as_i16, i16, "Try to get as i16 volume");
    impl_downcast!(as_i8, i8, "Try to get as i8 volume");
    impl_downcast!(as_u16, u16, "Try to get as u16 volume");
    impl_downcast!(as_complex_i16, Int16Complex, "Try to get as complex i16 volume");
    impl_downcast!(as_complex_f32, Float32Complex, "Try to get as complex f32 volume");
    impl_downcast!(as_packed4bit, Packed4Bit, "Try to get as Packed4Bit volume");

    /// Try to get as f16 volume
    #[cfg(feature = "f16")]
    pub fn as_f16(&self) -> Option<&Volume<half::f16, Vec<u8>>> {
        self.downcast_ref::<half::f16>()
    }
}
