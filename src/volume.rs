//! Volume container for MRC data

use crate::{AxisMap, Error, Header, Encoding, Voxel};

/// A 3D volume of voxel data
pub struct Volume<T, S> {
    header: Header,
    storage: S,
    _marker: core::marker::PhantomData<T>,
}

impl<T, S> Volume<T, S> {
    /// Get the header
    pub fn header(&self) -> &Header {
        &self.header
    }
    
    /// Get dimensions (nx, ny, nz)
    pub fn dimensions(&self) -> (usize, usize, usize) {
        self.header.dimensions()
    }
    
    /// Get the axis map
    pub fn axis_map(&self) -> &AxisMap {
        &self.header.axis_map
    }
    
    /// Total number of voxels
    pub fn len(&self) -> usize {
        self.header.voxel_count()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S> {
    /// Create a new volume from header and storage
    pub fn new(header: Header, storage: S) -> Result<Self, Error> {
        // Validate mode matches
        if T::MODE != header.mode {
            return Err(Error::TypeMismatch);
        }
        
        // Validate size
        let expected_size = header.voxel_count() * T::SIZE;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }
        
        Ok(Self {
            header,
            storage,
            _marker: core::marker::PhantomData,
        })
    }
    
    /// Get a voxel at linear index
    pub fn get(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.storage.as_ref()[offset..offset + T::SIZE])
    }
    
    /// Get a voxel at 3D coordinates
    pub fn get_at(&self, x: usize, y: usize, z: usize) -> T {
        let (nx, ny, _) = self.dimensions();
        let index = z * nx * ny + y * nx + x;
        self.get(index)
    }
    
    /// Iterate over all voxels
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian;
        let bytes = self.storage.as_ref();
        (0..self.len()).map(move |i| {
            let offset = i * T::SIZE;
            T::decode(endian, &bytes[offset..offset + T::SIZE])
        })
    }
}

impl<T: Voxel + Encoding, S: AsMut<[u8]>> Volume<T, S> {
    /// Set a voxel at linear index
    pub fn set(&mut self, index: usize, value: T) {
        let offset = index * T::SIZE;
        value.encode(self.header.file_endian, &mut self.storage.as_mut()[offset..offset + T::SIZE]);
    }
    
    /// Set a voxel at 3D coordinates
    pub fn set_at(&mut self, x: usize, y: usize, z: usize, value: T) {
        let (nx, ny, _) = self.dimensions();
        let index = z * nx * ny + y * nx + x;
        self.set(index, value);
    }
}
