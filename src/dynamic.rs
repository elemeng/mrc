//! Dynamic dispatch for runtime mode handling
//!
//! When the MRC mode is unknown at compile time, use `VolumeData`
//! to hold the volume with runtime type dispatch.

use crate::{Error, Header, Mode, Volume};
use crate::voxel::{Int16Complex, Float32Complex};
use alloc::vec::Vec;

/// Runtime-typed volume data
///
/// This enum provides runtime dispatch for MRC volumes when the
/// voxel type is not known at compile time.
#[derive(Debug)]
pub enum VolumeData {
    /// 8-bit signed integer (Mode 0)
    I8(Volume<i8, Vec<u8>>),
    /// 16-bit signed integer (Mode 1)
    I16(Volume<i16, Vec<u8>>),
    /// 32-bit float (Mode 2)
    F32(Volume<f32, Vec<u8>>),
    /// Complex 16-bit integer (Mode 3)
    ComplexI16(Volume<Int16Complex, Vec<u8>>),
    /// Complex 32-bit float (Mode 4)
    ComplexF32(Volume<Float32Complex, Vec<u8>>),
    /// 16-bit unsigned integer (Mode 6)
    U16(Volume<u16, Vec<u8>>),
    /// 16-bit float (Mode 12)
    #[cfg(feature = "f16")]
    F16(Volume<half::f16, Vec<u8>>),
}

impl VolumeData {
    /// Create VolumeData from raw bytes and header
    pub fn from_bytes(header: Header, data: Vec<u8>) -> Result<Self, Error> {
        match header.mode {
            Mode::Int8 => {
                let vol = Volume::new(header, data)?;
                Ok(Self::I8(vol))
            }
            Mode::Int16 => {
                let vol = Volume::new(header, data)?;
                Ok(Self::I16(vol))
            }
            Mode::Float32 => {
                let vol = Volume::new(header, data)?;
                Ok(Self::F32(vol))
            }
            Mode::Int16Complex => {
                let vol = Volume::new(header, data)?;
                Ok(Self::ComplexI16(vol))
            }
            Mode::Float32Complex => {
                let vol = Volume::new(header, data)?;
                Ok(Self::ComplexF32(vol))
            }
            Mode::Uint16 => {
                let vol = Volume::new(header, data)?;
                Ok(Self::U16(vol))
            }
            #[cfg(feature = "f16")]
            Mode::Float16 => {
                let vol = Volume::new(header, data)?;
                Ok(Self::F16(vol))
            }
            Mode::Packed4Bit => {
                // Packed4Bit requires special handling - decode to u8
                Err(Error::InvalidMode)
            }
        }
    }
    
    /// Get the mode of this volume
    pub fn mode(&self) -> Mode {
        match self {
            Self::I8(_) => Mode::Int8,
            Self::I16(_) => Mode::Int16,
            Self::F32(_) => Mode::Float32,
            Self::ComplexI16(_) => Mode::Int16Complex,
            Self::ComplexF32(_) => Mode::Float32Complex,
            Self::U16(_) => Mode::Uint16,
            #[cfg(feature = "f16")]
            Self::F16(_) => Mode::Float16,
        }
    }
    
    /// Get the header
    pub fn header(&self) -> &Header {
        match self {
            Self::I8(v) => v.header(),
            Self::I16(v) => v.header(),
            Self::F32(v) => v.header(),
            Self::ComplexI16(v) => v.header(),
            Self::ComplexF32(v) => v.header(),
            Self::U16(v) => v.header(),
            #[cfg(feature = "f16")]
            Self::F16(v) => v.header(),
        }
    }
    
    /// Get dimensions (nx, ny, nz)
    pub fn dimensions(&self) -> (usize, usize, usize) {
        match self {
            Self::I8(v) => v.dimensions(),
            Self::I16(v) => v.dimensions(),
            Self::F32(v) => v.dimensions(),
            Self::ComplexI16(v) => v.dimensions(),
            Self::ComplexF32(v) => v.dimensions(),
            Self::U16(v) => v.dimensions(),
            #[cfg(feature = "f16")]
            Self::F16(v) => v.dimensions(),
        }
    }
    
    /// Total number of voxels
    pub fn len(&self) -> usize {
        match self {
            Self::I8(v) => v.len(),
            Self::I16(v) => v.len(),
            Self::F32(v) => v.len(),
            Self::ComplexI16(v) => v.len(),
            Self::ComplexF32(v) => v.len(),
            Self::U16(v) => v.len(),
            #[cfg(feature = "f16")]
            Self::F16(v) => v.len(),
        }
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Try to get as f32 volume
    pub fn as_f32(&self) -> Option<&Volume<f32, Vec<u8>>> {
        match self {
            Self::F32(v) => Some(v),
            _ => None,
        }
    }
    
    /// Try to get as i16 volume
    pub fn as_i16(&self) -> Option<&Volume<i16, Vec<u8>>> {
        match self {
            Self::I16(v) => Some(v),
            _ => None,
        }
    }
    
    /// Try to get as u16 volume
    pub fn as_u16(&self) -> Option<&Volume<u16, Vec<u8>>> {
        match self {
            Self::U16(v) => Some(v),
            _ => None,
        }
    }
    
    /// Convert to f32 values (allocates)
    pub fn to_f32_vec(&self) -> Option<Vec<f32>> {
        match self {
            Self::F32(v) => Some(v.iter().collect()),
            Self::I16(v) => Some(v.iter().map(|x| x as f32).collect()),
            Self::U16(v) => Some(v.iter().map(|x| x as f32).collect()),
            #[cfg(feature = "f16")]
            Self::F16(v) => Some(v.iter().map(|x| x.to_f32()).collect()),
            _ => None,
        }
    }
}
