//! Volume container for MRC data
//!
//! Generic volume container with compile-time type safety.

use crate::{AxisMap, Error, Header, Encoding, Voxel};

#[cfg(feature = "std")]
use alloc::vec::Vec;

/// A volume of voxel data with configurable dimensionality
///
/// # Type Parameters
/// - `T`: Voxel type (must implement Voxel + Encoding)
/// - `S`: Storage backend (must implement AsRef<[u8]> for read, AsMut<[u8]> for write)
/// - `D`: Dimensionality (default 3 for 3D volumes)
#[derive(Debug)]
pub struct Volume<T, S, const D: usize = 3> {
    header: Header,
    storage: S,
    shape: [usize; D],
    strides: [usize; D],
    _marker: core::marker::PhantomData<T>,
}

impl<T, S, const D: usize> Volume<T, S, D> {
    /// Get the header
    pub fn header(&self) -> &Header {
        &self.header
    }
    
    /// Get the shape
    pub fn shape(&self) -> &[usize; D] {
        &self.shape
    }
    
    /// Get the strides
    pub fn strides(&self) -> &[usize; D] {
        &self.strides
    }
    
    /// Get the axis map
    pub fn axis_map(&self) -> &AxisMap {
        &self.header.axis_map
    }
    
    /// Total number of voxels
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S, 3> {
    /// Create a new 3D volume from header and storage
    pub fn new(header: Header, storage: S) -> Result<Self, Error> {
        // Validate mode matches
        if T::MODE != header.mode {
            return Err(Error::TypeMismatch);
        }
        
        let shape = [header.nx, header.ny, header.nz];
        let total = shape[0] * shape[1] * shape[2];
        
        // Validate size
        let expected_size = total * T::SIZE;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }
        
        // Calculate strides (C-order: column-major)
        let strides = [1, shape[0], shape[0] * shape[1]];
        
        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }
    
    /// Create from dimensions and data
    pub fn from_data(nx: usize, ny: usize, nz: usize, endian: crate::FileEndian, storage: S) -> Result<Self, Error> {
        let shape = [nx, ny, nz];
        let total = nx * ny * nz;
        let strides = [1, nx, nx * ny];
        
        let expected_size = total * T::SIZE;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }
        
        let header = Header {
            nx, ny, nz,
            mode: T::MODE,
            file_endian: endian,
            ..Default::default()
        };
        
        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }
    
    /// Get dimensions as tuple
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.shape[0], self.shape[1], self.shape[2])
    }
    
    /// Get a voxel at linear index
    pub fn get(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.storage.as_ref()[offset..offset + T::SIZE])
    }
    
    /// Get a voxel at 3D coordinates
    pub fn get_at(&self, x: usize, y: usize, z: usize) -> T {
        let index = z * self.strides[2] + y * self.strides[1] + x * self.strides[0];
        self.get(index)
    }
    
    /// Iterate over all voxels
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian;
        let bytes = self.storage.as_ref();
        let len = self.len();
        (0..len).map(move |i| {
            let offset = i * T::SIZE;
            T::decode(endian, &bytes[offset..offset + T::SIZE])
        })
    }
    
    /// Get slice of raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.storage.as_ref()
    }
}

impl<T: Voxel + Encoding, S: AsMut<[u8]>> Volume<T, S, 3> {
    /// Get mutable access to raw bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.storage.as_mut()
    }
    
    /// Set a voxel at linear index
    pub fn set(&mut self, index: usize, value: T) {
        let offset = index * T::SIZE;
        value.encode(self.header.file_endian, &mut self.storage.as_mut()[offset..offset + T::SIZE]);
    }
    
    /// Set a voxel at 3D coordinates
    pub fn set_at(&mut self, x: usize, y: usize, z: usize, value: T) {
        let index = z * self.strides[2] + y * self.strides[1] + x * self.strides[0];
        self.set(index, value);
    }
}

/// 2D volume (image slice)
pub type Image2D<T, S> = Volume<T, S, 2>;

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S, 2> {
    /// Create a new 2D image from storage
    pub fn new_2d(nx: usize, ny: usize, endian: crate::FileEndian, storage: S) -> Result<Self, Error> {
        let shape = [nx, ny];
        let total = nx * ny;
        let strides = [1, nx];
        
        let expected_size = total * T::SIZE;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }
        
        let header = Header {
            nx, ny, nz: 1,
            mode: T::MODE,
            file_endian: endian,
            ..Default::default()
        };
        
        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }
    
    /// Get a pixel at 2D coordinates
    pub fn get_pixel(&self, x: usize, y: usize) -> T {
        let index = y * self.strides[1] + x * self.strides[0];
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.storage.as_ref()[offset..offset + T::SIZE])
    }
}

// Type aliases for common volume types

/// Volume with Vec<u8> storage (most common)
pub type VecVolume<T, const D: usize = 3> = Volume<T, Vec<u8>, D>;

/// Volume with memory-mapped storage
#[cfg(feature = "mmap")]
pub type MmapVolume<T, const D: usize = 3> = Volume<T, memmap2::Mmap, D>;