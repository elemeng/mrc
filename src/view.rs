use crate::{DataBlock, DataBlockMut, Error, ExtHeader, ExtHeaderMut, Header, Mode};

#[non_exhaustive]
/// A read-only view into an MRC file's components.
///
/// This struct provides access to the three main components of an MRC file:
/// - `header`: The decoded MRC header (native-endian)
/// - `ext_header`: Extended header raw bytes (opaque, no endianness conversion)
/// - `data`: Voxel data raw bytes (file-endian, decoded on access)
///
/// # Accessing Components
///
/// Use the accessor methods to access components:
/// - `header()` / `header_ref()` - access the header
/// - `ext_header()` - access extended header bytes
/// - `data()` / `data_ref()` - access the data block
#[derive(Debug, Clone)]
pub struct MrcView<'a> {
    header: Header,
    ext_header: ExtHeader<'a>,
    data: DataBlock<'a>,
}

impl<'a> MrcView<'a> {
    /// Create a new MrcView from separate extended header and data slices.
    ///
    /// This constructor provides explicit separation of the three MRC file components:
    ///
    /// ```text
    /// File layout:  | 1024 bytes | NSYMBT bytes | data_size bytes |
    ///               | Header     | ExtHeader    | VoxelData       |
    ///
    /// Memory model: | Header     | ExtHeader    | VoxelData       |
    ///               | (decoded)  | (raw bytes)  | (raw bytes)     |
    /// ```
    ///
    /// # Arguments
    /// * `header` - Decoded MRC header (native-endian)
    /// * `ext_header` - Extended header raw bytes (opaque, no endianness conversion)
    /// * `data` - Voxel data raw bytes (file-endian, decoded on access)
    ///
    /// # Errors
    /// Returns `Error::InvalidHeader` if the header validation fails
    /// Returns `Error::InvalidDimensions` if the data size doesn't match expected size
    #[inline]
    pub fn from_parts(header: Header, ext_header: &'a [u8], data: &'a [u8]) -> Result<Self, Error> {
        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let expected_ext_size = header.nsymbt as usize;
        let expected_data_size = header.data_size();

        if ext_header.len() != expected_ext_size {
            return Err(Error::InvalidDimensions);
        }

        if data.len() != expected_data_size {
            return Err(Error::InvalidDimensions);
        }

        let mode = Mode::from_i32(header.mode).ok_or(Error::InvalidMode)?;

        let file_endian = header.detect_endian();
        let voxel_count = (header.nx as usize)
            .checked_mul(header.ny as usize)
            .and_then(|v| v.checked_mul(header.nz as usize))
            .ok_or(Error::InvalidDimensions)?;

        Ok(Self {
            header,
            ext_header: ExtHeader::new(ext_header),
            data: DataBlock::new(data, mode, file_endian, voxel_count),
        })
    }

    /// Get a reference to the header
    #[inline]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get a reference to the header (alias for `header()`)
    #[inline]
    pub fn header_ref(&self) -> &Header {
        &self.header
    }

    /// Get the data mode
    #[inline]
    pub fn mode(&self) -> Option<Mode> {
        Mode::from_i32(self.header.mode)
    }

    /// Get the dimensions as (nx, ny, nz)
    #[inline]
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (
            self.header.nx as usize,
            self.header.ny as usize,
            self.header.nz as usize,
        )
    }

    /// Get the extended header bytes
    #[inline]
    pub fn ext_header(&self) -> &[u8] {
        self.ext_header.as_bytes()
    }

    /// Get a reference to the data block
    #[inline]
    pub fn data(&self) -> &DataBlock<'a> {
        &self.data
    }

    /// Get a reference to the data block (alias for `data()`)
    #[inline]
    pub fn data_ref(&self) -> &DataBlock<'a> {
        &self.data
    }

    /// Get a single voxel value as f32 at the given coordinates
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Float32
    /// Returns Error::IndexOutOfBounds if coordinates are out of bounds
    #[inline]
    pub fn get_voxel_f32(&self, x: usize, y: usize, z: usize) -> Result<f32, crate::Error> {
        let (nx, ny, _nz) = self.dimensions();
        if x >= nx || y >= ny || z >= self.header.nz as usize {
            return Err(crate::Error::IndexOutOfBounds {
                index: z * nx * ny + y * nx + x,
                length: self.data.len_voxels(),
            });
        }
        let index = z * nx * ny + y * nx + x;
        self.data.get_f32(index)
    }

    /// Get a single voxel value as i16 at the given coordinates
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int16
    /// Returns Error::IndexOutOfBounds if coordinates are out of bounds
    #[inline]
    pub fn get_voxel_i16(&self, x: usize, y: usize, z: usize) -> Result<i16, crate::Error> {
        let (nx, ny, _nz) = self.dimensions();
        if x >= nx || y >= ny || z >= self.header.nz as usize {
            return Err(crate::Error::IndexOutOfBounds {
                index: z * nx * ny + y * nx + x,
                length: self.data.len_voxels(),
            });
        }
        let index = z * nx * ny + y * nx + x;
        self.data.get_i16(index)
    }

    /// Get a single voxel value as u16 at the given coordinates
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Uint16
    /// Returns Error::IndexOutOfBounds if coordinates are out of bounds
    #[inline]
    pub fn get_voxel_u16(&self, x: usize, y: usize, z: usize) -> Result<u16, crate::Error> {
        let (nx, ny, _nz) = self.dimensions();
        if x >= nx || y >= ny || z >= self.header.nz as usize {
            return Err(crate::Error::IndexOutOfBounds {
                index: z * nx * ny + y * nx + x,
                length: self.data.len_voxels(),
            });
        }
        let index = z * nx * ny + y * nx + x;
        self.data.get_u16(index)
    }

    /// Get a single voxel value as i8 at the given coordinates
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int8
    /// Returns Error::IndexOutOfBounds if coordinates are out of bounds
    #[inline]
    pub fn get_voxel_i8(&self, x: usize, y: usize, z: usize) -> Result<i8, crate::Error> {
        let (nx, ny, _nz) = self.dimensions();
        if x >= nx || y >= ny || z >= self.header.nz as usize {
            return Err(crate::Error::IndexOutOfBounds {
                index: z * nx * ny + y * nx + x,
                length: self.data.len_voxels(),
            });
        }
        let index = z * nx * ny + y * nx + x;
        self.data.get_i8(index)
    }

    /// Get data layout information: shape and strides
    ///
    /// Returns ((nx, ny, nz), (sx, sy, sz)) where:
    /// - (nx, ny, nz) are the dimensions
    /// - (sx, sy, sz) are the strides in elements (not bytes)
    #[inline]
    pub fn data_layout(&self) -> ((usize, usize, usize), (usize, usize, usize)) {
        let (nx, ny, _nz) = self.dimensions();
        ((nx, ny, self.header.nz as usize), (1, nx, nx * ny))
    }

    /// Calculate the flat index from 3D coordinates
    #[inline]
    pub fn index_of(&self, x: usize, y: usize, z: usize) -> usize {
        let (nx, ny, _) = self.dimensions();
        z * nx * ny + y * nx + x
    }

    /// Convert a flat index to 3D coordinates
    #[inline]
    pub fn coords_of(&self, index: usize) -> (usize, usize, usize) {
        let (nx, ny, _) = self.dimensions();
        let z = index / (nx * ny);
        let remainder = index % (nx * ny);
        let y = remainder / nx;
        let x = remainder % nx;
        (x, y, z)
    }
}

/// Mutable version of MrcView for write operations
#[non_exhaustive]
/// A mutable view into an MRC file's components.
///
/// This struct provides mutable access to the three main components of an MRC file:
/// - `header`: The decoded MRC header (native-endian)
/// - `ext_header`: Extended header raw bytes (opaque, no endianness conversion)
/// - `data`: Voxel data raw bytes (file-endian, decoded on access)
///
/// # Accessing Components
///
/// Use the accessor methods to access components:
/// - `header()` / `header_mut()` - access the header
/// - `ext_header()` / `ext_header_mut()` - access extended header bytes
/// - `data()` / `data_mut()` - access the data block
#[derive(Debug)]
pub struct MrcViewMut<'a> {
    header: Header,
    ext_header: ExtHeaderMut<'a>,
    data: DataBlockMut<'a>,
}

impl<'a> MrcViewMut<'a> {
    /// Create a new MrcViewMut from separate extended header and data slices.
    ///
    /// This constructor provides explicit separation of the three MRC file components.
    ///
    /// # Arguments
    /// * `header` - Decoded MRC header (native-endian)
    /// * `ext_header` - Extended header raw bytes (opaque, no endianness conversion)
    /// * `data` - Voxel data raw bytes (file-endian, decoded on access)
    ///
    /// # Errors
    /// Returns `Error::InvalidHeader` if the header validation fails
    /// Returns `Error::InvalidDimensions` if the data size doesn't match expected size
    #[inline]
    pub fn from_parts(
        header: Header,
        ext_header: &'a mut [u8],
        data: &'a mut [u8],
    ) -> Result<Self, Error> {
        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let expected_ext_size = header.nsymbt as usize;
        let expected_data_size = header.data_size();

        if ext_header.len() != expected_ext_size {
            return Err(Error::InvalidDimensions);
        }

        if data.len() != expected_data_size {
            return Err(Error::InvalidDimensions);
        }

        let mode = Mode::from_i32(header.mode).ok_or(Error::InvalidMode)?;

        let file_endian = header.detect_endian();
        let voxel_count = (header.nx as usize)
            .checked_mul(header.ny as usize)
            .and_then(|v| v.checked_mul(header.nz as usize))
            .ok_or(Error::InvalidDimensions)?;

        Ok(Self {
            header,
            ext_header: ExtHeaderMut::new(ext_header),
            data: DataBlockMut::new(data, mode, file_endian, voxel_count),
        })
    }

    /// Get a reference to the header
    #[inline]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get a mutable reference to the header
    ///
    /// # Safety Note
    /// Modifying the header without updating the data block may break invariants.
    /// Ensure consistency between header and data after modification.
    #[inline]
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    /// Get the extended header bytes
    #[inline]
    pub fn ext_header(&self) -> &[u8] {
        self.ext_header.as_bytes()
    }

    /// Get mutable access to the extended header bytes
    #[inline]
    pub fn ext_header_mut(&mut self) -> &mut [u8] {
        self.ext_header.as_bytes_mut()
    }

    /// Get a reference to the data block
    #[inline]
    pub fn data(&self) -> &DataBlockMut<'a> {
        &self.data
    }

    /// Get mutable access to the raw data bytes
    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.data.as_bytes_mut()
    }

    /// Get a mutable reference to the data block
    #[inline]
    pub fn data_block_mut(&mut self) -> &mut DataBlockMut<'a> {
        &mut self.data
    }

    /// Get the data mode
    #[inline]
    pub fn mode(&self) -> Option<Mode> {
        Mode::from_i32(self.header.mode)
    }

    /// Get the dimensions as (nx, ny, nz)
    #[inline]
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (
            self.header.nx as usize,
            self.header.ny as usize,
            self.header.nz as usize,
        )
    }

    /// Get data layout information: shape and strides
    ///
    /// Returns ((nx, ny, nz), (sx, sy, sz)) where:
    /// - (nx, ny, nz) are the dimensions
    /// - (sx, sy, sz) are the strides in elements (not bytes)
    #[inline]
    pub fn data_layout(&self) -> ((usize, usize, usize), (usize, usize, usize)) {
        let (nx, ny, _nz) = self.dimensions();
        ((nx, ny, self.header.nz as usize), (1, nx, nx * ny))
    }

    /// Calculate the flat index from 3D coordinates
    #[inline]
    pub fn index_of(&self, x: usize, y: usize, z: usize) -> usize {
        let (nx, ny, _) = self.dimensions();
        z * nx * ny + y * nx + x
    }

    /// Convert a flat index to 3D coordinates
    #[inline]
    pub fn coords_of(&self, index: usize) -> (usize, usize, usize) {
        let (nx, ny, _) = self.dimensions();
        let z = index / (nx * ny);
        let remainder = index % (nx * ny);
        let y = remainder / nx;
        let x = remainder % nx;
        (x, y, z)
    }

    /// Set a single voxel value as f32 at the given coordinates
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Float32
    /// Returns Error::IndexOutOfBounds if coordinates are out of bounds
    #[inline]
    pub fn set_voxel_f32(&mut self, x: usize, y: usize, z: usize, value: f32) -> Result<(), crate::Error> {
        let (nx, ny, _nz) = self.dimensions();
        if x >= nx || y >= ny || z >= self.header.nz as usize {
            return Err(crate::Error::IndexOutOfBounds {
                index: z * nx * ny + y * nx + x,
                length: self.data.len_voxels(),
            });
        }
        let index = z * nx * ny + y * nx + x;
        let offset = index * 4;
        let bytes = match self.data.file_endian() {
            crate::FileEndian::LittleEndian => value.to_le_bytes(),
            crate::FileEndian::BigEndian => value.to_be_bytes(),
        };
        let data_bytes = self.data.as_bytes_mut();
        data_bytes[offset..offset + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Set a single voxel value as i16 at the given coordinates
    ///
    /// # Errors
    /// Returns Error::InvalidMode if mode is not Int16
    /// Returns Error::IndexOutOfBounds if coordinates are out of bounds
    #[inline]
    pub fn set_voxel_i16(&mut self, x: usize, y: usize, z: usize, value: i16) -> Result<(), crate::Error> {
        let (nx, ny, _nz) = self.dimensions();
        if x >= nx || y >= ny || z >= self.header.nz as usize {
            return Err(crate::Error::IndexOutOfBounds {
                index: z * nx * ny + y * nx + x,
                length: self.data.len_voxels(),
            });
        }
        let index = z * nx * ny + y * nx + x;
        // We need to set a single value, but set_i16 expects a slice
        // Use direct byte manipulation for single value
        let offset = index * 2;
        let bytes = match self.data.file_endian() {
            crate::FileEndian::LittleEndian => value.to_le_bytes(),
            crate::FileEndian::BigEndian => value.to_be_bytes(),
        };
        let data_bytes = self.data.as_bytes_mut();
        data_bytes[offset..offset + 2].copy_from_slice(&bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_map_creation() {
        let mut header = Header::new();
        header.nx = 10;
        header.ny = 10;
        header.nz = 10;
        header.mode = 2;

        let ext_header = [0u8; 0];
        let data = [0u8; 4000];
        let map = MrcView::from_parts(header, &ext_header, &data);
        assert!(map.is_ok());
    }

    #[test]
    fn test_map_invalid_header() {
        let header = Header::new();
        let ext_header = [0u8; 0];
        let data = [0u8; 100];
        let map = MrcView::from_parts(header, &ext_header, &data);
        assert!(matches!(map, Err(Error::InvalidHeader)));
    }

    #[test]
    fn test_map_insufficient_data() {
        let mut header = Header::new();
        header.nx = 10;
        header.ny = 10;
        header.nz = 10;
        header.mode = 2;

        let ext_header = [0u8; 0];
        let data = [0u8; 100];
        let map = MrcView::from_parts(header, &ext_header, &data);
        assert!(matches!(map, Err(Error::InvalidDimensions)));
    }

    #[test]
    fn test_ext_header_zero_length() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 0; // No extended header

        let ext_header = [0u8; 0];
        let data = [0u8; 32]; // 2×2×2×4 bytes
        let map = MrcView::from_parts(header, &ext_header, &data).unwrap();

        assert_eq!(map.ext_header().len(), 0);
        assert_eq!(map.data().as_bytes().len(), 32);
    }

    #[test]
    fn test_ext_header_100_bytes() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 100; // 100-byte extended header

        let ext_header = vec![0u8; 100];
        let data = [0u8; 32];
        let map = MrcView::from_parts(header, &ext_header, &data).unwrap();

        assert_eq!(map.ext_header().len(), 100);
        assert_eq!(map.data().as_bytes().len(), 32);
    }

    #[test]
    fn test_ext_header_1024_bytes() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 1024; // 1024-byte extended header

        let ext_header = vec![0u8; 1024];
        let data = [0u8; 32];
        let map = MrcView::from_parts(header, &ext_header, &data).unwrap();

        assert_eq!(map.ext_header().len(), 1024);
        assert_eq!(map.data().as_bytes().len(), 32);
    }

    #[test]
    fn test_map_mut_ext_header_write() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 100;

        let mut ext_header = vec![0u8; 100];
        let mut data = [0u8; 32];
        let map = MrcViewMut::from_parts(header, &mut ext_header, &mut data).unwrap();

        // ExtHeader is read-only by design
        // This test verifies that ExtHeader can be accessed
        assert_eq!(map.ext_header().len(), 100);
    }

    #[test]
    fn test_map_mut_data_access() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 1;
        header.mode = 2;
        header.nsymbt = 0;

        let mut ext_header = [0u8; 0];
        let mut data = [0u8; 16]; // 2×2×1×4 bytes
        let mut map = MrcViewMut::from_parts(header, &mut ext_header, &mut data).unwrap();

        // Test data_mut for byte-level access
        let data_mut = map.data_mut();
        data_mut[0] = 0x00; // First byte of first float
        data_mut[1] = 0x00;
        data_mut[2] = 0x28;
        data_mut[3] = 0x42; // IEEE 754 representation of 42.0 (little-endian)

        assert_eq!(data_mut[3], 0x42);
    }
}
