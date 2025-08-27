use crate::{Error, Header, Mode};

#[non_exhaustive]
pub struct MrcView<'a> {
    header: Header,
    data: &'a [u8],
    ext_header: &'a [u8],
}

impl<'a> MrcView<'a> {
    #[inline]
    pub fn new(header: Header, data: &'a [u8]) -> Result<Self, Error> {
        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let ext_header_size = header.nsymbt as usize;
        let expected_data_size = header.data_size();
        let total_expected = ext_header_size + expected_data_size;

        if data.len() < total_expected {
            return Err(Error::InvalidDimensions);
        }

        let (ext_header, data) = data.split_at(ext_header_size);

        Ok(Self {
            header,
            data,
            ext_header,
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
    pub fn view<T: bytemuck::Pod>(&self) -> Result<&[T], Error> {
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        // Use unchecked cast for performance - validated by data_size
        if data.len() % core::mem::size_of::<T>() != 0 {
            return Err(Error::TypeMismatch);
        }

        // SAFETY: We validated the size alignment and the data is contiguous
        let num_elements = data.len() / core::mem::size_of::<T>();
        let ptr = data.as_ptr() as *const T;
        Ok(unsafe { core::slice::from_raw_parts(ptr, num_elements) })
    }

    #[inline]
    pub fn slice_bytes(&self, range: core::ops::Range<usize>) -> Result<&[u8], Error> {
        // Use get_unchecked for performance when bounds are known
        if range.start > range.end || range.end > self.data.len() {
            return Err(Error::InvalidDimensions);
        }
        // SAFETY: We validated the bounds
        Ok(unsafe { self.data.get_unchecked(range) })
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        self.data
    }

    #[inline]
    pub fn data_aligned<T: bytemuck::Pod>(&self) -> Result<&[T], Error> {
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        // Check alignment for SIMD operations (cache-line aligned for 64-byte boundaries)
        let ptr = data.as_ptr() as usize;
        let align = core::mem::align_of::<T>();
        let cache_line_align = 64;
        let effective_align = align.max(cache_line_align);

        if ptr % effective_align != 0 {
            return Err(Error::TypeMismatch);
        }

        if data.len() % core::mem::size_of::<T>() != 0 {
            return Err(Error::TypeMismatch);
        }

        let num_elements = data.len() / core::mem::size_of::<T>();
        let ptr = data.as_ptr() as *const T;
        // SAFETY: We validated alignment and size
        Ok(unsafe { core::slice::from_raw_parts(ptr, num_elements) })
    }

    #[inline]
    pub fn ext_header(&self) -> &[u8] {
        self.ext_header
    }

    #[inline]
    pub fn save(&mut self, _path: &str) -> Result<(), Error> {
        // This would require file I/O, which is handled by backends
        Err(Error::Io)
    }

    #[inline]
    pub fn swap_endian<T: bytemuck::Pod + Copy>(&mut self) -> Result<(), Error> {
        // This method is available on MapMut, not Map
        Err(Error::Io)
    }

    #[inline]
    pub fn swap_endian_bytes(&self) -> Result<(), Error> {
        // Map is read-only, cannot swap bytes
        Err(Error::Io)
    }
}

/// Mutable version of MrcView for write operations
#[non_exhaustive]
pub struct MrcViewMut<'a> {
    header: Header,
    data: &'a mut [u8],
    ext_header: &'a mut [u8],
}

impl<'a> MrcViewMut<'a> {
    #[inline]
    pub fn new(header: Header, data: &'a mut [u8]) -> Result<Self, Error> {
        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let ext_header_size = header.nsymbt as usize;
        let expected_data_size = header.data_size();
        let total_expected = ext_header_size + expected_data_size;

        if data.len() < total_expected {
            return Err(Error::InvalidDimensions);
        }

        let (ext_header, data) = data.split_at_mut(ext_header_size);

        Ok(Self {
            header,
            data,
            ext_header,
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
        self.ext_header
    }

    #[inline]
    pub fn write_ext_header(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.ext_header.len() {
            return Err(Error::InvalidDimensions);
        }
        self.ext_header.copy_from_slice(data);
        Ok(())
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.data
    }

    #[inline]
    pub fn view_mut<T: bytemuck::Pod>(&mut self) -> Result<&mut [T], Error> {
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get_mut(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        if data.len() % core::mem::size_of::<T>() != 0 {
            return Err(Error::TypeMismatch);
        }

        let num_elements = data.len() / core::mem::size_of::<T>();
        let ptr = data.as_mut_ptr() as *mut T;
        Ok(unsafe { core::slice::from_raw_parts_mut(ptr, num_elements) })
    }

    #[inline]
    pub fn swap_endian_bytes(&mut self) -> Result<(), Error> {
        // Swap header endian
        self.header.swap_endian();

        // Swap data bytes based on mode
        match Mode::from_i32(self.header.mode) {
            Some(Mode::Int8) => {
                // 1-byte types don’t need swapping
                Ok(())
            }
            Some(Mode::Uint16) => {
                // 2-byte unsigned 16-bit → must swap
                let data = self.view_mut::<u16>()?;
                for val in data.iter_mut() {
                    *val = val.swap_bytes();
                }
                Ok(())
            }
            Some(Mode::Int16) | Some(Mode::Int16Complex) => {
                // 2-byte types
                let data = self.view_mut::<i16>()?;
                for val in data.iter_mut() {
                    *val = val.swap_bytes();
                }
                Ok(())
            }
            Some(Mode::Float32) | Some(Mode::Float32Complex) => {
                // 4-byte types
                let data = self.view_mut::<f32>()?;
                for val in data.iter_mut() {
                    let bytes = bytemuck::bytes_of_mut(val);
                    bytes.reverse();
                }
                Ok(())
            }
            Some(Mode::Float16) => {
                // 2-byte f16 types
                #[cfg(feature = "f16")]
                {
                    let data = self.view_mut::<half::f16>()?;
                    for val in data.iter_mut() {
                        let bytes = bytemuck::bytes_of_mut(val);
                        bytes.reverse();
                    }
                }
                #[cfg(not(feature = "f16"))]
                {
                    // Fallback to u16 when f16 feature is disabled
                    let data = self.view_mut::<u16>()?;
                    for val in data.iter_mut() {
                        *val = val.swap_bytes();
                    }
                }
                Ok(())
            }
            Some(Mode::Packed4Bit) => {
                // 4-bit packed data - no endian swapping needed for individual nibbles
                Ok(())
            }
            None => Err(Error::InvalidMode),
        }
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

        let data = [0u8; 4000];
        let map = MrcView::new(header, &data);
        assert!(map.is_ok());
    }

    #[test]
    fn test_map_invalid_header() {
        let header = Header::new();
        let data = [0u8; 100];
        let map = MrcView::new(header, &data);
        assert!(matches!(map, Err(Error::InvalidHeader)));
    }

    #[test]
    fn test_map_insufficient_data() {
        let mut header = Header::new();
        header.nx = 10;
        header.ny = 10;
        header.nz = 10;
        header.mode = 2;

        let data = [0u8; 100];
        let map = MrcView::new(header, &data);
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

        let data = [0u8; 32]; // 2×2×2×4 bytes
        let map = MrcView::new(header, &data).unwrap();

        assert_eq!(map.ext_header().len(), 0);
        assert_eq!(map.data().len(), 32);
    }

    #[test]
    fn test_ext_header_100_bytes() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 100; // 100-byte extended header

        let total_size = 100 + 32; // 100 bytes ext header + 32 bytes data
        let data = vec![0u8; total_size];
        let map = MrcView::new(header, &data).unwrap();

        assert_eq!(map.ext_header().len(), 100);
        assert_eq!(map.data().len(), 32);
    }

    #[test]
    fn test_ext_header_1024_bytes() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 1024; // 1024-byte extended header

        let total_size = 1024 + 32; // 1024 bytes ext header + 32 bytes data
        let data = vec![0u8; total_size];
        let map = MrcView::new(header, &data).unwrap();

        assert_eq!(map.ext_header().len(), 1024);
        assert_eq!(map.data().len(), 32);
    }

    #[test]
    fn test_map_mut_ext_header_write() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 100;

        let total_size = 100 + 32;
        let mut buffer = vec![0u8; total_size];
        let mut map = MrcViewMut::new(header, &mut buffer).unwrap();

        let test_data = vec![0xAAu8; 100];
        map.write_ext_header(&test_data).unwrap();

        assert_eq!(map.ext_header(), &test_data[..]);
    }

    #[test]
    fn test_map_mut_ext_header_length_mismatch() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 100;

        let total_size = 100 + 32;
        let mut buffer = vec![0u8; total_size];
        let mut map = MrcViewMut::new(header, &mut buffer).unwrap();

        let wrong_size = vec![0xAAu8; 50]; // Wrong length
        let result = map.write_ext_header(&wrong_size);
        assert!(matches!(result, Err(Error::InvalidDimensions)));
    }

    #[test]
    fn test_map_mut_data_access() {
        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 1;
        header.mode = 2;
        header.nsymbt = 0;

        let mut buffer = vec![0u8; 16]; // 2×2×1×4 bytes
        let mut map = MrcViewMut::new(header, &mut buffer).unwrap();

        let floats = map.view_mut::<f32>().unwrap();
        floats[0] = 42.0;

        assert_eq!(floats[0], 42.0);
    }

    #[test]
    fn test_endian_swap_header() {
        let mut header = Header::new();
        header.nx = 0x12345678;
        header.ny = 0x12345678u32 as i32; // Use valid i32 range
        header.mode = 2;

        header.swap_endian();

        assert_eq!(header.nx, 0x78563412);
        assert_eq!(header.ny, 0x78563412u32 as i32);
    }
}
