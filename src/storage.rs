//! Storage backends for MRC volume data

use crate::Error;

/// Storage backend trait for volume data
///
/// This trait abstracts over different storage mechanisms:
/// - `VecStorage`: In-memory vector storage
/// - `MmapStorage`: Memory-mapped file storage
pub trait Storage {
    /// The element type stored
    type Item: Copy;
    
    /// Get the number of elements
    fn len(&self) -> usize;
    
    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Get a slice of the data
    fn as_slice(&self) -> &[Self::Item];
    
    /// Get a mutable slice of the data
    fn as_slice_mut(&mut self) -> &mut [Self::Item];
}

/// In-memory vector storage
#[cfg(feature = "std")]
pub struct VecStorage<T> {
    data: alloc::vec::Vec<T>,
}

#[cfg(feature = "std")]
impl<T: Copy> VecStorage<T> {
    /// Create new storage with given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: alloc::vec::Vec::with_capacity(capacity),
        }
    }
    
    /// Create storage from a vector
    pub fn from_vec(data: alloc::vec::Vec<T>) -> Self {
        Self { data }
    }
    
    /// Convert to vector
    pub fn into_vec(self) -> alloc::vec::Vec<T> {
        self.data
    }
}

#[cfg(feature = "std")]
impl<T: Copy> Storage for VecStorage<T> {
    type Item = T;
    
    fn len(&self) -> usize {
        self.data.len()
    }
    
    fn as_slice(&self) -> &[T] {
        &self.data
    }
    
    fn as_slice_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

/// Memory-mapped file storage (requires mmap feature)
#[cfg(feature = "mmap")]
pub struct MmapStorage<T> {
    mmap: memmap2::Mmap,
    _marker: core::marker::PhantomData<T>,
}

#[cfg(feature = "mmap")]
impl<T: Copy + bytemuck::Pod> MmapStorage<T> {
    /// Create from a memory map
    pub fn new(mmap: memmap2::Mmap) -> Result<Self, Error> {
        // Verify alignment and size
        if mmap.len() % core::mem::size_of::<T>() != 0 {
            return Err(Error::InvalidDimensions);
        }
        
        // Check alignment
        let ptr = mmap.as_ptr();
        if ptr.align_offset(core::mem::align_of::<T>()) != 0 {
            return Err(Error::MisalignedData {
                required: core::mem::align_of::<T>(),
                actual: 0,
            });
        }
        
        Ok(Self {
            mmap,
            _marker: core::marker::PhantomData,
        })
    }
}

#[cfg(feature = "mmap")]
impl<T: Copy + bytemuck::Pod> Storage for MmapStorage<T> {
    type Item = T;
    
    fn len(&self) -> usize {
        self.mmap.len() / core::mem::size_of::<T>()
    }
    
    fn as_slice(&self) -> &[T] {
        bytemuck::cast_slice(&self.mmap)
    }
    
    fn as_slice_mut(&mut self) -> &mut [T] {
        // This requires MmapMut, not Mmap
        // For now, panic - we'd need a separate MmapStorageMut type
        unimplemented!("MmapStorage is read-only; use MmapStorageMut for mutable access")
    }
}