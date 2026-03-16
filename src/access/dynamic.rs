//! Dynamic dispatch for runtime mode handling

use super::Volume;
use crate::core::{Error, Mode};
use crate::header::Header;
use crate::voxel::{ComplexF32, ComplexI16, Encoding, Packed4Bit, Voxel};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Internal trait for type-erased volume operations
trait DynVolume {
    fn header(&self) -> &Header;
    fn mode(&self) -> Mode;
    fn dimensions(&self) -> (usize, usize, usize);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn as_any(&self) -> &dyn core::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn core::any::Any;
}

impl<T: Voxel + Encoding> DynVolume for Volume<T, Vec<u8>> {
    fn header(&self) -> &Header {
        Volume::header(self)
    }
    fn mode(&self) -> Mode {
        self.header().mode()
    }
    fn dimensions(&self) -> (usize, usize, usize) {
        Volume::dimensions(self)
    }
    fn len(&self) -> usize {
        Volume::len(self)
    }
    fn is_empty(&self) -> bool {
        Volume::is_empty(self)
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
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
            Mode::Int16Complex => Box::new(Volume::<ComplexI16, Vec<u8>>::new(header, data)?),
            Mode::Float32Complex => Box::new(Volume::<ComplexF32, Vec<u8>>::new(header, data)?),
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
    ///
    /// # Example
    /// ```ignore
    /// if let Some(vol) = data.downcast_ref::<f32>() {
    ///     // work with f32 volume
    /// }
    /// ```
    pub fn downcast_ref<T: Voxel + Encoding>(&self) -> Option<&Volume<T, Vec<u8>>> {
        self.0.as_any().downcast_ref::<Volume<T, Vec<u8>>>()
    }

    /// Downcast to mutable typed volume
    ///
    /// # Example
    /// ```ignore
    /// if let Some(vol) = data.downcast_mut::<f32>() {
    ///     // modify f32 volume
    /// }
    /// ```
    pub fn downcast_mut<T: Voxel + Encoding>(&mut self) -> Option<&mut Volume<T, Vec<u8>>> {
        self.0.as_any_mut().downcast_mut::<Volume<T, Vec<u8>>>()
    }

    /// Get an iterator over f32 values if the volume is Float32 mode
    ///
    /// Returns `None` if the volume is not Float32 mode.
    ///
    /// # Example
    /// ```ignore
    /// if let Some(iter) = data.iter_f32() {
    ///     let sum: f32 = iter.sum();
    /// }
    /// ```
    pub fn iter_f32(&self) -> Option<impl Iterator<Item = f32> + '_> {
        let vol = self.downcast_ref::<f32>()?;
        Some(vol.iter())
    }

    /// Get an iterator over i8 values if the volume is Int8 mode
    ///
    /// Returns `None` if the volume is not Int8 mode.
    pub fn iter_i8(&self) -> Option<impl Iterator<Item = i8> + '_> {
        let vol = self.downcast_ref::<i8>()?;
        Some(vol.iter())
    }

    /// Get an iterator over i16 values if the volume is Int16 mode
    ///
    /// Returns `None` if the volume is not Int16 mode.
    pub fn iter_i16(&self) -> Option<impl Iterator<Item = i16> + '_> {
        let vol = self.downcast_ref::<i16>()?;
        Some(vol.iter())
    }

    /// Get an iterator over u16 values if the volume is Uint16 mode
    ///
    /// Returns `None` if the volume is not Uint16 mode.
    pub fn iter_u16(&self) -> Option<impl Iterator<Item = u16> + '_> {
        let vol = self.downcast_ref::<u16>()?;
        Some(vol.iter())
    }

    /// Get an iterator over ComplexI16 values if the volume is Int16Complex mode
    ///
    /// Returns `None` if the volume is not Int16Complex mode.
    pub fn iter_complex_i16(&self) -> Option<impl Iterator<Item = ComplexI16> + '_> {
        let vol = self.downcast_ref::<ComplexI16>()?;
        Some(vol.iter())
    }

    /// Get an iterator over ComplexF32 values if the volume is Float32Complex mode
    ///
    /// Returns `None` if the volume is not Float32Complex mode.
    pub fn iter_complex_f32(&self) -> Option<impl Iterator<Item = ComplexF32> + '_> {
        let vol = self.downcast_ref::<ComplexF32>()?;
        Some(vol.iter())
    }
}
