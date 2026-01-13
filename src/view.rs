use crate::{EncodeToFile, Error, Header, Mode};

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

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

    /// Decode data as f32 values, handling endianness conversion
    ///
    /// This method allocates a new Vec<f32> and converts bytes according to the file's endianness.
    /// The result is always native-endian and safe for mathematical operations.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32 (mode 2)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_f32(&self) -> Result<Vec<f32>, Error> {
        if self.header.mode != 2 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len() / 4);
        let chunks: Vec<_> = data.chunks_exact(4).collect();

        for chunk in chunks {
            let value = crate::DecodeFromFile::decode(file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as i16 values, handling endianness conversion
    ///
    /// This method allocates a new Vec<i16> and converts bytes according to the file's endianness.
    /// The result is always native-endian and safe for mathematical operations.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16 (mode 1)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_i16(&self) -> Result<Vec<i16>, Error> {
        if self.header.mode != 1 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len() / 2);
        let chunks: Vec<_> = data.chunks_exact(2).collect();

        for chunk in chunks {
            let value = crate::DecodeFromFile::decode(file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as u16 values, handling endianness conversion
    ///
    /// This method allocates a new Vec<u16> and converts bytes according to the file's endianness.
    /// The result is always native-endian and safe for mathematical operations.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Uint16 (mode 6)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_u16(&self) -> Result<Vec<u16>, Error> {
        if self.header.mode != 6 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len() / 2);
        let chunks: Vec<_> = data.chunks_exact(2).collect();

        for chunk in chunks {
            let value = crate::DecodeFromFile::decode(file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as i8 values
    ///
    /// This method allocates a new Vec<i8>. No endianness conversion is needed for 1-byte values.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int8 (mode 0)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_i8(&self) -> Result<Vec<i8>, Error> {
        if self.header.mode != 0 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len());
        for byte in data {
            let value = crate::DecodeFromFile::decode(file_endian, &[*byte]);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Int16Complex values, handling endianness conversion
    ///
    /// This method allocates a new Vec<crate::Int16Complex> and converts bytes according to the file's endianness.
    /// The result is always native-endian and safe for mathematical operations.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16Complex (mode 3)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_int16_complex(&self) -> Result<Vec<crate::Int16Complex>, Error> {
        if self.header.mode != 3 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len() / 4);
        let chunks: Vec<_> = data.chunks_exact(4).collect();

        for chunk in chunks {
            let value = crate::DecodeFromFile::decode(file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Float32Complex values, handling endianness conversion
    ///
    /// This method allocates a new Vec<crate::Float32Complex> and converts bytes according to the file's endianness.
    /// The result is always native-endian and safe for mathematical operations.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32Complex (mode 4)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_float32_complex(&self) -> Result<Vec<crate::Float32Complex>, Error> {
        if self.header.mode != 4 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len() / 8);
        let chunks: Vec<_> = data.chunks_exact(8).collect();

        for chunk in chunks {
            let value = crate::DecodeFromFile::decode(file_endian, chunk);
            result.push(value);
        }

        Ok(result)
    }

    /// Decode data as Packed4Bit values
    ///
    /// This method allocates a new Vec<crate::Packed4Bit>. Two 4-bit values are packed into each byte.
    /// No endianness conversion is needed for 1-byte values.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Packed4Bit (mode 101)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_packed4bit(&self) -> Result<Vec<crate::Packed4Bit>, Error> {
        if self.header.mode != 101 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len());
        for byte in data {
            let value = crate::DecodeFromFile::decode(file_endian, &[*byte]);
            result.push(value);
        }

        Ok(result)
    }

    #[cfg(feature = "f16")]
    /// Decode data as f16 values, handling endianness conversion
    ///
    /// This method allocates a new Vec<half::f16> and converts bytes according to the file's endianness.
    /// The result is always native-endian and safe for mathematical operations.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float16 (mode 12)
    /// Returns Error::InvalidDimensions if the data size doesn't match expected dimensions
    pub fn data_as_f16(&self) -> Result<Vec<half::f16>, Error> {
        if self.header.mode != 12 {
            return Err(Error::InvalidMode);
        }

        let file_endian = self.header.detect_endian();
        let expected_size = self.header.data_size();
        let data = self
            .data
            .get(..expected_size)
            .ok_or(Error::InvalidDimensions)?;

        let mut result = Vec::with_capacity(data.len() / 2);
        let chunks: Vec<_> = data.chunks_exact(2).collect();

        for chunk in chunks {
            let value = crate::DecodeFromFile::decode(file_endian, chunk);
            result.push(value);
        }

        Ok(result)
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

    /// Encode f32 values to data, handling endianness conversion
    ///
    /// This method writes f32 values to the data buffer, converting from native
    /// endian to the file's endianness.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32 (mode 2)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), Error> {
        if self.header.mode != 2 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() * 4 != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i * 4..i * 4 + 4]);
        }

        Ok(())
    }

    /// Encode i16 values to data, handling endianness conversion
    ///
    /// This method writes i16 values to the data buffer, converting from native
    /// endian to the file's endianness.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16 (mode 1)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_i16(&mut self, values: &[i16]) -> Result<(), Error> {
        if self.header.mode != 1 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() * 2 != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i * 2..i * 2 + 2]);
        }

        Ok(())
    }

    /// Encode u16 values to data, handling endianness conversion
    ///
    /// This method writes u16 values to the data buffer, converting from native
    /// endian to the file's endianness.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Uint16 (mode 6)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_u16(&mut self, values: &[u16]) -> Result<(), Error> {
        if self.header.mode != 6 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() * 2 != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i * 2..i * 2 + 2]);
        }

        Ok(())
    }

    /// Encode i8 values to data
    ///
    /// This method writes i8 values to the data buffer. No endianness conversion
    /// is needed for 1-byte values.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int8 (mode 0)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_i8(&mut self, values: &[i8]) -> Result<(), Error> {
        if self.header.mode != 0 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i..i + 1]);
        }

        Ok(())
    }

    /// Encode Int16Complex values to data, handling endianness conversion
    ///
    /// This method writes Int16Complex values to the data buffer, converting from native
    /// endian to the file's endianness.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Int16Complex (mode 3)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_int16_complex(&mut self, values: &[crate::Int16Complex]) -> Result<(), Error> {
        if self.header.mode != 3 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() * 4 != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i * 4..i * 4 + 4]);
        }

        Ok(())
    }

    /// Encode Float32Complex values to data, handling endianness conversion
    ///
    /// This method writes Float32Complex values to the data buffer, converting from native
    /// endian to the file's endianness.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float32Complex (mode 4)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_float32_complex(&mut self, values: &[crate::Float32Complex]) -> Result<(), Error> {
        if self.header.mode != 4 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() * 8 != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i * 8..i * 8 + 8]);
        }

        Ok(())
    }

    /// Encode Packed4Bit values to data
    ///
    /// This method writes Packed4Bit values to the data buffer. Two 4-bit values are packed into each byte.
    /// No endianness conversion is needed for 1-byte values.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Packed4Bit (mode 101)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_packed4bit(&mut self, values: &[crate::Packed4Bit]) -> Result<(), Error> {
        if self.header.mode != 101 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i..i + 1]);
        }

        Ok(())
    }

    #[cfg(feature = "f16")]
    /// Encode f16 values to data, handling endianness conversion
    ///
    /// This method writes f16 values to the data buffer, converting from native
    /// endian to the file's endianness.
    ///
    /// # Errors
    /// Returns Error::InvalidMode if the file mode is not Float16 (mode 12)
    /// Returns Error::InvalidDimensions if the data size doesn't match the input length
    pub fn write_f16(&mut self, values: &[half::f16]) -> Result<(), Error> {
        if self.header.mode != 12 {
            return Err(Error::InvalidMode);
        }

        let expected_size = self.header.data_size();
        if self.data.len() != expected_size || values.len() * 2 != expected_size {
            return Err(Error::InvalidDimensions);
        }

        let file_endian = self.header.detect_endian();
        for (i, &value) in values.iter().enumerate() {
            value.encode(file_endian, &mut self.data[i * 2..i * 2 + 2]);
        }

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

        // Test data_mut for byte-level access
        let data = map.data_mut();
        data[0] = 0x00; // First byte of first float
        data[1] = 0x00;
        data[2] = 0x28;
        data[3] = 0x42; // IEEE 754 representation of 42.0 (little-endian)

        assert_eq!(data[3], 0x42);
    }

    }
