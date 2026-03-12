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
    /// Strides for linear indexing (accounts for axis_map)
    strides: [usize; D],
    _marker: core::marker::PhantomData<T>,
}

impl<T, S, const D: usize> Volume<T, S, D> {
    /// Get the header
    pub fn header(&self) -> &Header {
        &self.header
    }
    
    /// Get the shape (logical dimensions: nx, ny, nz)
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
    /// 
    /// The strides are calculated from the axis_map in the header,
    /// which defines the storage order of the data.
    pub fn new(header: Header, storage: S) -> Result<Self, Error> {
        // Validate mode matches
        if T::MODE != header.mode {
            return Err(Error::TypeMismatch);
        }
        
        let shape = [header.nx, header.ny, header.nz];
        let total = shape[0].checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or(Error::InvalidDimensions)?;
        
        // Validate size
        let expected_size = total.checked_mul(T::SIZE)
            .ok_or(Error::InvalidDimensions)?;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }
        
        // Calculate strides based on axis_map
        // MRC data is stored in column-major order (column varies fastest)
        // axis_map tells us which logical dimension (X, Y, Z) corresponds to each storage axis
        let strides = calculate_strides(&header.axis_map, shape);
        
        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }
    
    /// Create from dimensions and data (standard axis map)
    pub fn from_data(nx: usize, ny: usize, nz: usize, endian: crate::FileEndian, storage: S) -> Result<Self, Error> {
        let shape = [nx, ny, nz];
        let total = nx.checked_mul(ny)
            .and_then(|v| v.checked_mul(nz))
            .ok_or(Error::InvalidDimensions)?;
        
        let expected_size = total.checked_mul(T::SIZE)
            .ok_or(Error::InvalidDimensions)?;
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
        
        // Standard strides for X=column, Y=row, Z=section
        let strides = [1, nx, nx * ny];
        
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
    /// 
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn get(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        T::decode(self.header.file_endian, &bytes[offset..offset + T::SIZE])
    }
    
    /// Get a voxel at linear index, returning None if out of bounds
    pub fn get_checked(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        Some(T::decode(self.header.file_endian, &bytes[offset..offset + T::SIZE]))
    }
    
    /// Get a voxel at logical 3D coordinates (x, y, z)
    /// 
    /// The coordinates are in logical space (X, Y, Z). The stride calculation
    /// accounts for the axis_map to correctly access the stored data.
    /// 
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get_at(&self, x: usize, y: usize, z: usize) -> T {
        let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
        self.get(index)
    }
    
    /// Get a voxel at 3D coordinates, returning None if out of bounds
    pub fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<T> {
        if x >= self.shape[0] || y >= self.shape[1] || z >= self.shape[2] {
            return None;
        }
        Some(self.get_at(x, y, z))
    }
    
    /// Iterate over all voxels in storage order
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian;
        let bytes = self.storage.as_ref();
        let len = self.len();
        (0..len).map(move |i| {
            let offset = i * T::SIZE;
            T::decode(endian, &bytes[offset..offset + T::SIZE])
        })
    }
    
    /// Iterate over voxels in logical order (X varies fastest)
    pub fn iter_logical(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian;
        let bytes = self.storage.as_ref();
        let (nx, ny, nz) = self.dimensions();
        let strides = self.strides;
        
        (0..nz).flat_map(move |z| {
            (0..ny).flat_map(move |y| {
                (0..nx).map(move |x| {
                    let index = x * strides[0] + y * strides[1] + z * strides[2];
                    let offset = index * T::SIZE;
                    T::decode(endian, &bytes[offset..offset + T::SIZE])
                })
            })
        })
    }
    
    /// Get slice of raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.storage.as_ref()
    }
    
    /// Convert linear index to logical coordinates (x, y, z)
    /// 
    /// This assumes standard C-order indexing and may not be correct
    /// for non-standard axis_map values.
    pub fn coords_of(&self, index: usize) -> (usize, usize, usize) {
        let (nx, ny, _) = self.dimensions();
        let z = index / (nx * ny);
        let remainder = index % (nx * ny);
        let y = remainder / nx;
        let x = remainder % nx;
        (x, y, z)
    }
    
    /// Convert logical coordinates to linear index
    pub fn index_of(&self, x: usize, y: usize, z: usize) -> usize {
        x * self.strides[0] + y * self.strides[1] + z * self.strides[2]
    }
}

impl<T: Voxel + Encoding, S: AsMut<[u8]>> Volume<T, S, 3> {
    /// Get mutable access to raw bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.storage.as_mut()
    }
    
    /// Set a voxel at linear index
    /// 
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn set(&mut self, index: usize, value: T) {
        let offset = index * T::SIZE;
        let bytes = self.storage.as_mut();
        value.encode(self.header.file_endian, &mut bytes[offset..offset + T::SIZE]);
    }
    
    /// Set a voxel at linear index, returning error if out of bounds
    pub fn set_checked(&mut self, index: usize, value: T) -> Result<(), Error> {
        if index >= self.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len(),
            });
        }
        self.set(index, value);
        Ok(())
    }
    
    /// Set a voxel at logical 3D coordinates (x, y, z)
    /// 
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn set_at(&mut self, x: usize, y: usize, z: usize, value: T) {
        let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
        self.set(index, value);
    }
    
    /// Set a voxel at 3D coordinates, returning error if out of bounds
    pub fn set_at_checked(&mut self, x: usize, y: usize, z: usize, value: T) -> Result<(), Error> {
        if x >= self.shape[0] || y >= self.shape[1] || z >= self.shape[2] {
            return Err(Error::IndexOutOfBounds {
                index: x + y * self.shape[0] + z * self.shape[0] * self.shape[1],
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }
}

/// Calculate strides for 3D indexing based on axis_map
/// 
/// MRC data is stored in column-major order where:
/// - Column (storage axis 0) varies fastest
/// - Row (storage axis 1) varies medium
/// - Section (storage axis 2) varies slowest
/// 
/// The axis_map tells us which logical dimension (X, Y, or Z) is stored
/// as column, row, or section.
/// 
/// Returns strides for (X, Y, Z) logical coordinates.
fn calculate_strides(axis_map: &AxisMap, shape: [usize; 3]) -> [usize; 3] {
    // Storage dimensions: column, row, section
    let storage_strides = [1, shape[0], shape[0] * shape[1]];
    
    // Map logical dimensions to storage strides
    // axis_map.column tells us which logical dim (1=X, 2=Y, 3=Z) is column
    // axis_map.row tells us which logical dim is row
    // axis_map.section tells us which logical dim is section
    
    let mut strides = [0usize; 3];
    
    // Column varies fastest - assign its stride to the logical dimension stored as column
    strides[(axis_map.column - 1) as usize] = storage_strides[0];
    
    // Row varies medium
    strides[(axis_map.row - 1) as usize] = storage_strides[1];
    
    // Section varies slowest
    strides[(axis_map.section - 1) as usize] = storage_strides[2];
    
    strides
}

/// 2D volume (image slice)
pub type Image2D<T, S> = Volume<T, S, 2>;

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S, 2> {
    /// Create a new 2D image from storage
    pub fn new_2d(nx: usize, ny: usize, endian: crate::FileEndian, storage: S) -> Result<Self, Error> {
        let shape = [nx, ny];
        let total = nx.checked_mul(ny)
            .ok_or(Error::InvalidDimensions)?;
        
        let expected_size = total.checked_mul(T::SIZE)
            .ok_or(Error::InvalidDimensions)?;
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
        
        let strides = [1, nx];
        
        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }
    
    /// Get a pixel at 2D coordinates
    /// 
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get_pixel(&self, x: usize, y: usize) -> T {
        let index = y * self.strides[1] + x * self.strides[0];
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.storage.as_ref()[offset..offset + T::SIZE])
    }
    
    /// Get a pixel at 2D coordinates, returning None if out of bounds
    pub fn get_pixel_checked(&self, x: usize, y: usize) -> Option<T> {
        if x >= self.shape[0] || y >= self.shape[1] {
            return None;
        }
        Some(self.get_pixel(x, y))
    }
}

// Type aliases for common volume types

/// Volume with Vec<u8> storage (most common)
pub type VecVolume<T, const D: usize = 3> = Volume<T, Vec<u8>, D>;

/// Volume with memory-mapped storage (read-only)
#[cfg(feature = "mmap")]
pub type MmapVolume<T, const D: usize = 3> = Volume<T, memmap2::Mmap, D>;

/// Volume with mutable memory-mapped storage
#[cfg(feature = "mmap")]
pub type MmapVolumeMut<T, const D: usize = 3> = Volume<T, memmap2::MmapMut, D>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_standard_axis_map_strides() {
        // Standard: X=column, Y=row, Z=section
        let axis_map = AxisMap::new(1, 2, 3);
        let shape = [64, 64, 64];
        let strides = calculate_strides(&axis_map, shape);
        
        // X should have stride 1 (column, fastest)
        // Y should have stride 64 (row)
        // Z should have stride 4096 (section, slowest)
        assert_eq!(strides, [1, 64, 4096]);
    }
    
    #[test]
    fn test_nonstandard_axis_map_strides() {
        // Non-standard: Z=column, Y=row, X=section
        let axis_map = AxisMap::new(3, 2, 1);
        let shape = [64, 64, 64];
        let strides = calculate_strides(&axis_map, shape);
        
        // X (stored as section) should have stride 4096
        // Y (stored as row) should have stride 64
        // Z (stored as column) should have stride 1
        assert_eq!(strides, [4096, 64, 1]);
    }
}
