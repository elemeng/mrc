//! Integration tests for MRC library

#[cfg(test)]
mod header_tests {
    use mrc::{Header, RawHeader, Mode};
    
    #[test]
    fn test_raw_header_size() {
        assert_eq!(core::mem::size_of::<RawHeader>(), 1024);
    }
    
    #[test]
    fn test_mode_from_i32() {
        assert_eq!(Mode::from_i32(0), Some(Mode::Int8));
        assert_eq!(Mode::from_i32(1), Some(Mode::Int16));
        assert_eq!(Mode::from_i32(2), Some(Mode::Float32));
        assert_eq!(Mode::from_i32(3), Some(Mode::Int16Complex));
        assert_eq!(Mode::from_i32(4), Some(Mode::Float32Complex));
        assert_eq!(Mode::from_i32(6), Some(Mode::Uint16));
        assert_eq!(Mode::from_i32(12), Some(Mode::Float16));
        assert_eq!(Mode::from_i32(5), None);
        assert_eq!(Mode::from_i32(-1), None);
    }
    
    #[test]
    fn test_mode_byte_size() {
        assert_eq!(Mode::Int8.byte_size(), 1);
        assert_eq!(Mode::Int16.byte_size(), 2);
        assert_eq!(Mode::Float32.byte_size(), 4);
        assert_eq!(Mode::Int16Complex.byte_size(), 4);
        assert_eq!(Mode::Float32Complex.byte_size(), 8);
        assert_eq!(Mode::Uint16.byte_size(), 2);
        assert_eq!(Mode::Float16.byte_size(), 2);
    }
    
    #[test]
    fn test_mode_properties() {
        assert!(Mode::Int16Complex.is_complex());
        assert!(Mode::Float32Complex.is_complex());
        assert!(!Mode::Int8.is_complex());
        assert!(!Mode::Float32.is_complex());
        
        assert!(Mode::Float32.is_float());
        assert!(Mode::Float32Complex.is_float());
        assert!(Mode::Float16.is_float());
        assert!(!Mode::Int8.is_float());
    }
    
    #[test]
    fn test_raw_header_new() {
        let header = RawHeader::new();
        assert!(header.has_valid_map());
        assert!(header.is_valid_mode());
        assert_eq!(header.nversion, 20140);
    }
    
    #[test]
    fn test_header_default() {
        let header = Header::default();
        assert_eq!(header.dimensions(), (1, 1, 1));
        assert_eq!(header.mode, Mode::Float32);
    }
    
    #[test]
    fn test_header_data_size() {
        let mut header = Header::default();
        header.nx = 10;
        header.ny = 20;
        header.nz = 30;
        
        header.mode = Mode::Int8;
        assert_eq!(header.data_size(), 10 * 20 * 30);
        
        header.mode = Mode::Int16;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 2);
        
        header.mode = Mode::Float32;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 4);
        
        header.mode = Mode::Int16Complex;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 4);
        
        header.mode = Mode::Float32Complex;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 8);
    }
    
    #[test]
    fn test_raw_header_data_size() {
        let mut header = RawHeader::new();
        header.nx = 10;
        header.ny = 20;
        header.nz = 30;
        
        header.mode = 2; // Float32
        assert_eq!(header.data_size(), 10 * 20 * 30 * 4);
    }
    
    #[test]
    fn test_raw_to_validated_header() {
        let mut raw = RawHeader::new();
        raw.nx = 64;
        raw.ny = 64;
        raw.nz = 64;
        raw.mode = 2; // Float32
        
        let header = Header::try_from(raw).unwrap();
        assert_eq!(header.dimensions(), (64, 64, 64));
        assert_eq!(header.mode, Mode::Float32);
    }
    
    #[test]
    fn test_validated_to_raw_header() {
        let mut header = Header::default();
        header.nx = 128;
        header.ny = 128;
        header.nz = 128;
        
        let raw: RawHeader = header.into();
        assert_eq!(raw.nx, 128);
        assert_eq!(raw.ny, 128);
        assert_eq!(raw.nz, 128);
    }
}

#[cfg(test)]
mod axis_tests {
    use mrc::AxisMap;
    
    #[test]
    fn test_axis_map_default() {
        let map = AxisMap::default();
        assert!(map.is_standard());
        assert!(map.validate());
    }
    
    #[test]
    fn test_axis_map_validation() {
        // Valid permutations
        assert!(AxisMap::new(1, 2, 3).validate());
        assert!(AxisMap::new(3, 2, 1).validate());
        
        // Invalid: not a permutation
        assert!(!AxisMap::new(1, 1, 2).validate());
        assert!(!AxisMap::new(1, 2, 1).validate());
    }
}

#[cfg(test)]
mod voxel_tests {
    use mrc::{Voxel, ScalarVoxel, RealVoxel, ComplexVoxel, ComplexI16, ComplexF32};
    
    #[test]
    fn test_voxel_bounds() {
        assert_eq!(i8::MIN, i8::MIN);
        assert_eq!(i8::MAX, i8::MAX);
        assert_eq!(f32::MIN, f32::NEG_INFINITY);
        assert_eq!(f32::MAX, f32::INFINITY);
    }
    
    #[test]
    fn test_real_voxel() {
        let f: f32 = 42.0;
        assert_eq!(f.to_f32(), 42.0);
        assert_eq!(f32::from_f32(100.0), 100.0);
    }
    
    #[test]
    fn test_complex_types() {
        let c = ComplexI16::new(10, 20);
        assert_eq!(c.re, 10);
        assert_eq!(c.im, 20);
        
        let cf = ComplexF32::new(1.0, 2.0);
        assert_eq!(cf.re, 1.0);
        assert_eq!(cf.im, 2.0);
    }
}

#[cfg(test)]
mod encoding_tests {
    use mrc::{Encoding, FileEndian, ComplexI16, ComplexF32};
    
    #[test]
    fn test_f32_encoding() {
        let value: f32 = 42.5;
        let mut bytes = [0u8; 4];
        value.encode(FileEndian::Little, &mut bytes);
        
        let decoded = f32::decode(FileEndian::Little, &bytes);
        assert_eq!(decoded, value);
    }
    
    #[test]
    fn test_i16_encoding() {
        let value: i16 = -1000;
        let mut bytes = [0u8; 2];
        value.encode(FileEndian::Little, &mut bytes);
        
        let decoded = i16::decode(FileEndian::Little, &bytes);
        assert_eq!(decoded, value);
    }
    
    #[test]
    fn test_complex_i16_encoding() {
        let value = ComplexI16::new(100, -200);
        let mut bytes = [0u8; 4];
        value.encode(FileEndian::Little, &mut bytes);
        
        let decoded = ComplexI16::decode(FileEndian::Little, &bytes);
        assert_eq!(decoded.re, 100);
        assert_eq!(decoded.im, -200);
    }
    
    #[test]
    fn test_complex_f32_encoding() {
        let value = ComplexF32::new(1.5, -2.5);
        let mut bytes = [0u8; 8];
        value.encode(FileEndian::Little, &mut bytes);
        
        let decoded = ComplexF32::decode(FileEndian::Little, &bytes);
        assert_eq!(decoded.re, 1.5);
        assert_eq!(decoded.im, -2.5);
    }
}

#[cfg(all(test, feature = "std"))]
mod io_tests {
    use mrc::{Header, MrcReader, MrcWriter, Mode, RawHeader};
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_write_and_read_header() {
        let mut temp = NamedTempFile::new().unwrap();
        
        // Create header
        let mut header = Header::default();
        header.nx = 64;
        header.ny = 64;
        header.nz = 32;
        header.mode = Mode::Float32;
        header.xlen = 100.0;
        header.ylen = 100.0;
        header.zlen = 50.0;
        
        // Write file
        let mut writer = MrcWriter::create(temp.path(), header.clone()).unwrap();
        
        // Write data
        let data = vec![0u8; header.data_size()];
        writer.write_data(&data).unwrap();
        
        // Read back
        let reader = MrcReader::open(temp.path()).unwrap();
        let read_header = reader.header();
        
        assert_eq!(read_header.dimensions(), (64, 64, 32));
        assert_eq!(read_header.mode, Mode::Float32);
    }
}