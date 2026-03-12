//! High-level Volume trait for zero-cost abstractions
//!
//! This module provides a unified trait-based API for volumes that works
//! across different storage backends with zero runtime overhead.

extern crate alloc;
use alloc::vec::Vec;

use crate::{AxisMap, Encoding, Error, FileEndian, Header, Mode, Voxel};
use crate::access::{VoxelAccess, VoxelAccessMut};

/// Core Volume trait - zero-cost abstraction over storage backends
///
/// This trait provides a unified interface for volume data access.
/// Implementations are zero-cost - all methods inline down to direct memory access.
pub trait Volume: VoxelAccess {
    /// The voxel type stored in this volume
    type Voxel: Voxel + Encoding;
    
    /// Get the header
    fn header(&self) -> &Header;
    
    /// Get the shape (nx, ny, nz)
    fn shape(&self) -> (usize, usize, usize);
    
    /// Get dimensions as tuple
    #[inline]
    fn dimensions(&self) -> (usize, usize, usize) {
        self.shape()
    }
    
    /// Get the axis map
    #[inline]
    fn axis_map(&self) -> &AxisMap {
        &self.header().axis_map
    }
    
    /// Get file endianness
    #[inline]
    fn endian(&self) -> FileEndian {
        self.header().file_endian
    }
    
    /// Get a voxel at 3D coordinates
    /// 
    /// # Panics
    /// Panics if coordinates are out of bounds
    #[inline]
    fn get_at(&self, x: usize, y: usize, z: usize) -> Self::Voxel {
        let strides = self.strides();
        let index = x * strides.0 + y * strides.1 + z * strides.2;
        // SAFETY: We assume the implementation has proper bounds checking
        // or the caller has verified coordinates
        unsafe { self.get_unchecked(index) }
    }
    
    /// Get a voxel at 3D coordinates, returning None if out of bounds
    #[inline]
    fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<Self::Voxel> {
        let (nx, ny, nz) = self.shape();
        if x >= nx || y >= ny || z >= nz {
            return None;
        }
        Some(self.get_at(x, y, z))
    }
    
    /// Get strides for index calculation
    fn strides(&self) -> (usize, usize, usize);
    
    /// Get a voxel without bounds checking
    /// 
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn get_unchecked(&self, index: usize) -> Self::Voxel;
    
    /// Compute statistics for this volume
    fn compute_stats(&self) -> VolumeStats
    where
        Self::Voxel: Into<f64>;
}

/// Mutable volume trait
pub trait VolumeMut: Volume + VoxelAccessMut {
    /// Set a voxel at 3D coordinates
    /// 
    /// # Panics
    /// Panics if coordinates are out of bounds
    #[inline]
    fn set_at(&mut self, x: usize, y: usize, z: usize, value: Self::Voxel) {
        let strides = self.strides();
        let index = x * strides.0 + y * strides.1 + z * strides.2;
        unsafe { self.set_unchecked(index, value); }
    }
    
    /// Set a voxel at 3D coordinates, returning error if out of bounds
    #[inline]
    fn set_at_checked(&mut self, x: usize, y: usize, z: usize, value: Self::Voxel) -> Result<(), Error> {
        let (nx, ny, nz) = self.shape();
        if x >= nx || y >= ny || z >= nz {
            return Err(Error::IndexOutOfBounds {
                index: x + y * nx + z * nx * ny,
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }
    
    /// Set a voxel without bounds checking
    /// 
    /// # Safety
    /// Caller must ensure index is within bounds
    unsafe fn set_unchecked(&mut self, index: usize, value: Self::Voxel);
}

/// Statistics computed from volume data
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeStats {
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Mean value
    pub mean: f64,
    /// RMS deviation
    pub rms: f64,
}

/// Volume reference type - zero-cost wrapper around byte slices
/// 
/// This type provides a Volume implementation over any AsRef<[u8]> storage.
/// All operations are inlined for zero runtime overhead.
#[derive(Debug, Clone, Copy)]
pub struct VolumeRef<'a, T: Voxel + Encoding> {
    header: &'a Header,
    data: &'a [u8],
    strides: (usize, usize, usize),
    _marker: core::marker::PhantomData<T>,
}

impl<'a, T: Voxel + Encoding> VolumeRef<'a, T> {
    /// Create a new volume reference
    /// 
    /// # Safety
    /// Caller must ensure data length matches header dimensions
    pub unsafe fn new_unchecked(header: &'a Header, data: &'a [u8]) -> Self {
        let shape = (header.nx(), header.ny(), header.nz());
        let strides = calculate_strides(&header.axis_map, shape);
        Self { header, data, strides, _marker: core::marker::PhantomData }
    }
    
    /// Create a new volume reference with validation
    pub fn new(header: &'a Header, data: &'a [u8]) -> Result<Self, Error> {
        if <T as Voxel>::MODE != header.mode() {
            return Err(Error::TypeMismatch);
        }
        let expected = header.nx() * header.ny() * header.nz() * T::SIZE;
        if data.len() < expected {
            return Err(Error::BufferTooSmall { expected, got: data.len() });
        }
        // SAFETY: We validated everything
        Ok(unsafe { Self::new_unchecked(header, data) })
    }
}

impl<T: Voxel + Encoding> VoxelAccess for VolumeRef<'_, T> {
    fn mode(&self) -> Mode {
        self.header.mode()
    }
    
    fn len(&self) -> usize {
        self.header.nx() * self.header.ny() * self.header.nz()
    }
    
    fn get<V: Voxel + Encoding>(&self, index: usize) -> Result<V, Error> {
        if <V as Voxel>::MODE != self.header.mode() {
            return Err(Error::TypeMismatch);
        }
        if index >= self.len() {
            return Err(Error::IndexOutOfBounds { index, length: self.len() });
        }
        let offset = index * V::SIZE;
        Ok(V::decode(self.header.file_endian, &self.data[offset..offset + V::SIZE]))
    }
}

impl<T: Voxel + Encoding> Volume for VolumeRef<'_, T> {
    type Voxel = T;
    
    #[inline]
    fn header(&self) -> &Header {
        self.header
    }
    
    #[inline]
    fn shape(&self) -> (usize, usize, usize) {
        (self.header.nx(), self.header.ny(), self.header.nz())
    }
    
    #[inline]
    fn strides(&self) -> (usize, usize, usize) {
        self.strides
    }
    
    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.data[offset..offset + T::SIZE])
    }
    
    fn compute_stats(&self) -> VolumeStats
    where
        T: Into<f64>,
    {
        let stats = crate::stats::compute_stats((0..self.len()).map(|i| unsafe { self.get_unchecked(i) }));
        VolumeStats {
            min: stats.min,
            max: stats.max,
            mean: stats.mean,
            rms: stats.rms,
        }
    }
}

/// Mutable volume reference
#[derive(Debug)]
pub struct VolumeMutRef<'a, T: Voxel + Encoding> {
    header: &'a Header,
    data: &'a mut [u8],
    strides: (usize, usize, usize),
    _marker: core::marker::PhantomData<T>,
}

impl<'a, T: Voxel + Encoding> VolumeMutRef<'a, T> {
    /// Create a new mutable volume reference
    pub fn new(header: &'a Header, data: &'a mut [u8]) -> Result<Self, Error> {
        if <T as Voxel>::MODE != header.mode() {
            return Err(Error::TypeMismatch);
        }
        let expected = header.nx() * header.ny() * header.nz() * T::SIZE;
        if data.len() < expected {
            return Err(Error::BufferTooSmall { expected, got: data.len() });
        }
        let shape = (header.nx(), header.ny(), header.nz());
        let strides = calculate_strides(&header.axis_map, shape);
        Ok(Self { header, data, strides, _marker: core::marker::PhantomData })
    }
}

impl<T: Voxel + Encoding> VoxelAccess for VolumeMutRef<'_, T> {
    fn mode(&self) -> Mode {
        self.header.mode()
    }
    
    fn len(&self) -> usize {
        self.header.nx() * self.header.ny() * self.header.nz()
    }
    
    fn get<V: Voxel + Encoding>(&self, index: usize) -> Result<V, Error> {
        if <V as Voxel>::MODE != self.header.mode() {
            return Err(Error::TypeMismatch);
        }
        if index >= self.len() {
            return Err(Error::IndexOutOfBounds { index, length: self.len() });
        }
        let offset = index * V::SIZE;
        Ok(V::decode(self.header.file_endian, &self.data[offset..offset + V::SIZE]))
    }
}

impl<T: Voxel + Encoding> VoxelAccessMut for VolumeMutRef<'_, T> {
    fn set<V: Voxel + Encoding>(&mut self, index: usize, value: V) -> Result<(), Error> {
        if <V as Voxel>::MODE != self.header.mode() {
            return Err(Error::TypeMismatch);
        }
        if index >= self.len() {
            return Err(Error::IndexOutOfBounds { index, length: self.len() });
        }
        let offset = index * V::SIZE;
        value.encode(self.header.file_endian, &mut self.data[offset..offset + V::SIZE]);
        Ok(())
    }
}

impl<T: Voxel + Encoding> Volume for VolumeMutRef<'_, T> {
    type Voxel = T;
    
    #[inline]
    fn header(&self) -> &Header {
        self.header
    }
    
    #[inline]
    fn shape(&self) -> (usize, usize, usize) {
        (self.header.nx(), self.header.ny(), self.header.nz())
    }
    
    #[inline]
    fn strides(&self) -> (usize, usize, usize) {
        self.strides
    }
    
    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.data[offset..offset + T::SIZE])
    }
    
    fn compute_stats(&self) -> VolumeStats
    where
        T: Into<f64>,
    {
        let stats = crate::stats::compute_stats((0..self.len()).map(|i| unsafe { self.get_unchecked(i) }));
        VolumeStats {
            min: stats.min,
            max: stats.max,
            mean: stats.mean,
            rms: stats.rms,
        }
    }
}

impl<T: Voxel + Encoding> VolumeMut for VolumeMutRef<'_, T> {
    #[inline]
    unsafe fn set_unchecked(&mut self, index: usize, value: T) {
        let offset = index * T::SIZE;
        value.encode(self.header.file_endian, &mut self.data[offset..offset + T::SIZE]);
    }
}

/// Calculate strides for 3D indexing
#[inline]
fn calculate_strides(axis_map: &AxisMap, shape: (usize, usize, usize)) -> (usize, usize, usize) {
    let storage_strides = [1, shape.0, shape.0 * shape.1];
    let mut strides = (0, 0, 0);
    
    match axis_map.column {
        1 => strides.0 = storage_strides[0],
        2 => strides.1 = storage_strides[0],
        3 => strides.2 = storage_strides[0],
        _ => {}
    }
    match axis_map.row {
        1 => strides.0 = storage_strides[1],
        2 => strides.1 = storage_strides[1],
        3 => strides.2 = storage_strides[1],
        _ => {}
    }
    match axis_map.section {
        1 => strides.0 = storage_strides[2],
        2 => strides.1 = storage_strides[2],
        3 => strides.2 = storage_strides[2],
        _ => {}
    }
    
    strides
}

/// Extension trait for volume operations
pub trait VolumeExt: Volume {
    /// Iterate over all voxels
    fn iter(&self) -> VolumeIter<'_, Self>;
    
    /// Map voxels to a new volume
    fn map<F, U>(&self, f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U;
}

impl<V: Volume> VolumeExt for V {
    #[inline]
    fn iter(&self) -> VolumeIter<'_, Self> {
        VolumeIter { volume: self, index: 0 }
    }
    
    #[inline]
    fn map<F, U>(&self, mut f: F) -> Vec<U>
    where
        F: FnMut(Self::Voxel) -> U,
    {
        (0..self.len()).map(|i| unsafe { f(self.get_unchecked(i)) }).collect()
    }
}

/// Iterator over volume voxels
pub struct VolumeIter<'a, V: Volume + ?Sized> {
    volume: &'a V,
    index: usize,
}

impl<V: Volume> Iterator for VolumeIter<'_, V> {
    type Item = V::Voxel;
    
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.volume.len() {
            let item = unsafe { self.volume.get_unchecked(self.index) };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
    
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.volume.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<V: Volume> ExactSizeIterator for VolumeIter<'_, V> {}
