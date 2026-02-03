use crate::{DataBlock, DataBlockMut, Error, ExtHeader, Header, Mode};

#[non_exhaustive]
/// A read-only view into an MRC file's components.
///
/// This struct provides access to the three main components of an MRC file:
/// - `header`: The decoded MRC header (native-endian)
/// - `ext_header`: Extended header raw bytes (opaque, no endianness conversion)
/// - `data`: Voxel data raw bytes (file-endian, decoded on access)
///
/// # Mutation Invariants
///
/// The `pub` fields allow direct access for convenience, but direct mutation
/// may break invariants. For example:
/// - Replacing `header` with a different header without updating `data` may
///   cause dimension mismatches
/// - The `data` block's endianness must match the header's detected endianness
///
/// When modifying views, ensure consistency between all components or use
/// the provided accessor methods when available.
#[derive(Debug, Clone)]
pub struct MrcView<'a> {
    pub header: Header,
    pub ext_header: ExtHeader<'a>,
    pub data: DataBlock<'a>,
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

        Ok(Self {
            header,
            ext_header: ExtHeader::new(ext_header),
            data: DataBlock::new(data, mode, file_endian),
        })
    }

    #[inline]
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    pub fn mode(&self) -> Option<Mode> {
        Mode::from_i32(self.header.mode)
    }

    #[inline]
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (
            self.header.nx as usize,
            self.header.ny as usize,
            self.header.nz as usize,
        )
    }

    #[inline]
    pub fn ext_header(&self) -> &[u8] {
        self.ext_header.as_bytes()
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
/// # Mutation Invariants
///
/// The `pub` fields allow direct access for convenience, but direct mutation
/// may break invariants. For example:
/// - Replacing `header` with a different header without updating `data` may
///   cause dimension mismatches
/// - The `data` block's endianness must match the header's detected endianness
/// - When writing back to file, the header must be re-encoded to file endianness
///
/// When modifying views, ensure consistency between all components or use
/// the provided accessor methods when available.
#[derive(Debug)]
pub struct MrcViewMut<'a> {
    pub header: Header,
    pub ext_header: ExtHeader<'a>,
    pub data: DataBlockMut<'a>,
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

        Ok(Self {
            header,
            ext_header: ExtHeader::new(ext_header),
            data: DataBlockMut::new(data, mode, file_endian),
        })
    }

    #[inline]
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    #[inline]
    pub fn ext_header(&self) -> &[u8] {
        self.ext_header.as_bytes()
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.data.as_bytes_mut()
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
        assert_eq!(map.data.as_bytes().len(), 32);
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
        assert_eq!(map.data.as_bytes().len(), 32);
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
        assert_eq!(map.data.as_bytes().len(), 32);
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
