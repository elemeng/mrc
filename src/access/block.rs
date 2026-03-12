//! DataBlock for fine-grained voxel access
//!
//! This module provides the `DataBlock` and `DataBlockMut` types for
//! type-safe access to voxel data with runtime mode checking.

extern crate alloc;

use crate::core::{Error, Mode, check_bounds};
use crate::voxel::{Encoding, FileEndian, Voxel, validate_mode};
use super::{VoxelAccess, VoxelAccessMut};

/// A read-only view into raw voxel data with runtime mode checking
///
/// `DataBlock` provides type-safe accessor methods for voxel data based on
/// the MRC mode. It handles endianness conversion transparently.
#[derive(Debug, Clone)]
pub struct DataBlock<'a> {
    data: &'a [u8],
    mode: Mode,
    endian: FileEndian,
    voxel_count: usize,
}

impl<'a> DataBlock<'a> {
    /// Create a new DataBlock
    pub fn new(data: &'a [u8], mode: Mode, endian: FileEndian, voxel_count: usize) -> Self {
        Self { data, mode, endian, voxel_count }
    }

    /// Get the data mode
    pub fn mode(&self) -> Mode { self.mode }

    /// Get the file endianness
    pub fn file_endian(&self) -> FileEndian { self.endian }

    /// Get the number of voxels
    pub fn len(&self) -> usize { self.voxel_count }

    /// Check if empty
    pub fn is_empty(&self) -> bool { self.voxel_count == 0 }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] { self.data }
}

impl VoxelAccess for DataBlock<'_> {
    fn mode(&self) -> Mode { self.mode }
    fn len(&self) -> usize { self.voxel_count }

    fn get<T: Voxel + Encoding>(&self, index: usize) -> Result<T, Error> {
        validate_mode::<T>(self.mode)?;
        check_bounds(index, self.voxel_count)?;
        let offset = index * T::SIZE;
        Ok(T::decode(self.endian, &self.data[offset..offset + T::SIZE]))
    }
}

/// A mutable view into raw voxel data with runtime mode checking
#[derive(Debug)]
pub struct DataBlockMut<'a> {
    data: &'a mut [u8],
    mode: Mode,
    endian: FileEndian,
    voxel_count: usize,
}

impl<'a> DataBlockMut<'a> {
    /// Create a new DataBlockMut
    pub fn new(data: &'a mut [u8], mode: Mode, endian: FileEndian, voxel_count: usize) -> Self {
        Self { data, mode, endian, voxel_count }
    }

    /// Get the data mode
    pub fn mode(&self) -> Mode { self.mode }

    /// Get the file endianness
    pub fn file_endian(&self) -> FileEndian { self.endian }

    /// Get the number of voxels
    pub fn len(&self) -> usize { self.voxel_count }

    /// Check if empty
    pub fn is_empty(&self) -> bool { self.voxel_count == 0 }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] { self.data }

    /// Get mutable access to the raw bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] { self.data }
}

impl VoxelAccess for DataBlockMut<'_> {
    fn mode(&self) -> Mode { self.mode }
    fn len(&self) -> usize { self.voxel_count }

    fn get<T: Voxel + Encoding>(&self, index: usize) -> Result<T, Error> {
        validate_mode::<T>(self.mode)?;
        check_bounds(index, self.voxel_count)?;
        let offset = index * T::SIZE;
        Ok(T::decode(self.endian, &self.data[offset..offset + T::SIZE]))
    }
}

impl VoxelAccessMut for DataBlockMut<'_> {
    fn set<T: Voxel + Encoding>(&mut self, index: usize, value: T) -> Result<(), Error> {
        validate_mode::<T>(self.mode)?;
        check_bounds(index, self.voxel_count)?;
        let offset = index * T::SIZE;
        value.encode(self.endian, &mut self.data[offset..offset + T::SIZE]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_datablock_i8() {
        let data = vec![0u8, 1, 2, 3];
        let block = DataBlock::new(&data, Mode::Int8, FileEndian::native(), 4);
        
        assert_eq!(block.get::<i8>(0).unwrap(), 0);
        assert_eq!(block.get::<i8>(1).unwrap(), 1);
        assert_eq!(block.get::<i8>(2).unwrap(), 2);
        assert_eq!(block.get::<i8>(3).unwrap(), 3);
    }

    #[test]
    fn test_datablock_i16() {
        let data = vec![0x01u8, 0x00, 0xFF, 0x7F];
        let block = DataBlock::new(&data, Mode::Int16, FileEndian::Little, 2);
        
        assert_eq!(block.get::<i16>(0).unwrap(), 1);
        assert_eq!(block.get::<i16>(1).unwrap(), 32767);
    }

    #[test]
    fn test_datablock_f32() {
        let data = 3.14159f32.to_le_bytes();
        let block = DataBlock::new(&data, Mode::Float32, FileEndian::Little, 1);
        
        let value: f32 = block.get(0).unwrap();
        assert!((value - 3.14159).abs() < 0.00001);
    }

    #[test]
    fn test_datablock_type_mismatch() {
        let data = vec![0u8; 4];
        let block = DataBlock::new(&data, Mode::Int8, FileEndian::native(), 4);
        
        assert!(matches!(block.get::<i16>(0), Err(Error::TypeMismatch)));
    }

    #[test]
    fn test_datablock_out_of_bounds() {
        let data = vec![0u8; 4];
        let block = DataBlock::new(&data, Mode::Int8, FileEndian::native(), 4);
        
        assert!(matches!(
            block.get::<i8>(10),
            Err(Error::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn test_datablock_packed4bit() {
        let data = vec![0xABu8, 0xCD];
        let block = DataBlock::new(&data, Mode::Packed4Bit, FileEndian::native(), 4);
        
        use crate::voxel::Packed4Bit;
        let p0: Packed4Bit = block.get(0).unwrap();
        assert_eq!(p0.first(), 0x0B);
        assert_eq!(p0.second(), 0x0A);
    }

    #[test]
    fn test_datablock_mut_i16() {
        let mut data = vec![0u8; 4];
        let mut block = DataBlockMut::new(&mut data, Mode::Int16, FileEndian::Little, 2);
        
        block.set(0, 1000i16).unwrap();
        block.set(1, -500i16).unwrap();
        
        assert_eq!(block.get::<i16>(0).unwrap(), 1000);
        assert_eq!(block.get::<i16>(1).unwrap(), -500);
    }

    #[test]
    fn test_datablock_mut_f32() {
        let mut data = vec![0u8; 4];
        let mut block = DataBlockMut::new(&mut data, Mode::Float32, FileEndian::Little, 1);
        
        block.set(0, 2.71828f32).unwrap();
        
        let value: f32 = block.get(0).unwrap();
        assert!((value - 2.71828).abs() < 0.00001);
    }
}
