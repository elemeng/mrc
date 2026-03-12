//! Storage backends for MRC volume data

use crate::Error;

#[cfg(feature = "std")]
extern crate alloc;

/// Storage backend trait for read-only volume data
///
/// This trait abstracts over different storage mechanisms:
/// - `VecStorage`: In-memory vector storage
/// - `MmapStorage`: Memory-mapped file storage (read-only)
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
}

/// Storage backend trait for mutable volume data
///
/// Types implementing this trait can be used for both reading and writing.
pub trait StorageMut: Storage {
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
    
    /// Push an element
    pub fn push(&mut self, value: T) {
        self.data.push(value);
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
}

#[cfg(feature = "std")]
impl<T: Copy> StorageMut for VecStorage<T> {
    fn as_slice_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}


/// Memory-mapped file storage (requires mmap feature)
/// 
/// This is read-only. For mutable memory-mapped storage, use `MmapStorageMut`.
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
    
    /// Open a file and create a memory-mapped storage
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        use std::fs::File;
        
        let file = File::open(path).map_err(Error::Io)?;
        let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(|_| Error::Mmap)?;
        Self::new(mmap)
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
}

// Note: MmapStorage does NOT implement StorageMut because it's read-only.
// Use MmapStorageMut for mutable access (would require MmapMut from memmap2).

#[cfg(feature = "mmap")]
pub use mmap_storage_mut::MmapStorageMut;

#[cfg(feature = "mmap")]
mod mmap_storage_mut {
    use crate::{Error, Storage, StorageMut};
    
    /// Mutable memory-mapped file storage (requires mmap feature)
    pub struct MmapStorageMut<T> {
        mmap: memmap2::MmapMut,
        _marker: core::marker::PhantomData<T>,
    }
    
    impl<T: Copy + bytemuck::Pod> MmapStorageMut<T> {
        /// Create from a mutable memory map
        pub fn new(mmap: memmap2::MmapMut) -> Result<Self, Error> {
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
        
        /// Open a file and create a mutable memory-mapped storage
        pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
            use std::fs::OpenOptions;
            
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(Error::Io)?;
            let mmap = unsafe { memmap2::MmapMut::map_mut(&file) }
                .map_err(|_| Error::Mmap)?;
            Self::new(mmap)
        }
        
        /// Flush changes to disk
        pub fn flush(&self) -> Result<(), Error> {
            self.mmap.flush().map_err(|_| Error::Mmap)
        }
    }
    
    impl<T: Copy + bytemuck::Pod> Storage for MmapStorageMut<T> {
        type Item = T;
        
        fn len(&self) -> usize {
            self.mmap.len() / core::mem::size_of::<T>()
        }
        
        fn as_slice(&self) -> &[T] {
            bytemuck::cast_slice(&self.mmap)
        }
    }
    
    impl<T: Copy + bytemuck::Pod> StorageMut for MmapStorageMut<T> {
        fn as_slice_mut(&mut self) -> &mut [T] {
            bytemuck::cast_slice_mut(&mut self.mmap)
        }
    }
}
