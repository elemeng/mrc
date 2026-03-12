//! Integration tests for MRC library

#[cfg(test)]
mod header_tests {
    use mrc::{Header, Mode};
    
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
    fn test_header_default() {
        let header = Header::default();
        assert_eq!(header.dimensions(), (1, 1, 1));
        assert_eq!(header.mode(), Mode::Float32);
    }
    
    #[test]
    fn test_header_builder() {
        let header = Header::builder()
            .dimensions(64, 64, 64)
            .mode(Mode::Int16)
            .origin(10.0, 20.0, 30.0)
            .build();
        
        assert_eq!(header.dimensions(), (64, 64, 64));
        assert_eq!(header.mode(), Mode::Int16);
        assert_eq!(header.xorigin(), 10.0);
    }
    
    #[test]
    fn test_header_data_size() {
        let header = Header::builder()
            .dimensions(10, 20, 30)
            .mode(Mode::Int8)
            .build();
        assert_eq!(header.data_size(), 10 * 20 * 30);
        
        let header = Header::builder()
            .dimensions(10, 20, 30)
            .mode(Mode::Int16)
            .build();
        assert_eq!(header.data_size(), 10 * 20 * 30 * 2);
        
        let header = Header::builder()
            .dimensions(10, 20, 30)
            .mode(Mode::Float32)
            .build();
        assert_eq!(header.data_size(), 10 * 20 * 30 * 4);
        
        let header = Header::builder()
            .dimensions(10, 20, 30)
            .mode(Mode::Float32Complex)
            .build();
        assert_eq!(header.data_size(), 10 * 20 * 30 * 8);
    }
    
    #[test]
    fn test_header_roundtrip() {
        let original = Header::builder()
            .dimensions(128, 128, 64)
            .mode(Mode::Float32)
            .origin(5.0, 10.0, 15.0)
            .statistics(0.0, 100.0, 50.0, 25.0)
            .build();

        let bytes = original.to_bytes();
        let parsed = Header::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.dimensions(), original.dimensions());
        assert_eq!(parsed.mode(), original.mode());
        assert_eq!(parsed.xorigin(), original.xorigin());
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
        assert!(AxisMap::new(1, 2, 3).validate());
        assert!(AxisMap::new(3, 2, 1).validate());
        
        assert!(!AxisMap::new(1, 1, 2).validate());
        assert!(!AxisMap::new(1, 2, 1).validate());
    }
}

#[cfg(test)]
mod voxel_tests {
    use mrc::{Voxel, ScalarVoxel, RealVoxel, ComplexVoxel, ComplexI16, ComplexF32, Mode};
    
    #[test]
    fn test_voxel_mode() {
        assert_eq!(i8::MODE, Mode::Int8);
        assert_eq!(i16::MODE, Mode::Int16);
        assert_eq!(f32::MODE, Mode::Float32);
        assert_eq!(ComplexI16::MODE, Mode::Int16Complex);
        assert_eq!(ComplexF32::MODE, Mode::Float32Complex);
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

#[cfg(all(test, feature = "std"))]
mod io_tests {
    use mrc::{Header, MrcReader, MrcWriter, Mode, Volume};
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_write_and_read_header() {
        let temp = NamedTempFile::new().unwrap();
        
        let header = Header::builder()
            .dimensions(64, 64, 32)
            .mode(Mode::Float32)
            .cell_dimensions(100.0, 100.0, 50.0)
            .build();
        
        let mut writer = MrcWriter::create(temp.path(), header.clone()).unwrap();
        
        let data = vec![0u8; header.data_size()];
        writer.write_data(&data).unwrap();
        
        let reader = MrcReader::open(temp.path()).unwrap();
        let read_header = reader.header();
        
        assert_eq!(read_header.dimensions(), (64, 64, 32));
        assert_eq!(read_header.mode(), Mode::Float32);
    }
    
    #[test]
    fn test_volume_roundtrip() {
        let temp = NamedTempFile::new().unwrap();
        
        // Create volume with builder
        let volume: Volume<f32, _> = Volume::builder()
            .dimensions(4, 4, 4)
            .voxel_size(1.0, 1.0, 1.0)
            .build_allocated();
        
        // Write
        let header = volume.header().clone();
        let mut writer = MrcWriter::create(temp.path(), header).unwrap();
        writer.write_data(volume.as_bytes()).unwrap();
        drop(writer);
        
        // Read back
        let mut reader = MrcReader::open(temp.path()).unwrap();
        let read_volume: Volume<f32, _> = reader.read_volume().unwrap();
        
        assert_eq!(read_volume.dimensions(), (4, 4, 4));
    }
    
    #[test]
    fn test_ext_header() {
        let temp = NamedTempFile::new().unwrap();
        
        let header = Header::builder()
            .dimensions(2, 2, 2)
            .mode(Mode::Float32)
            .extended_header_size(100)
            .build();
        
        let ext_header = vec![0xABu8; 100];
        let mut writer = MrcWriter::create_with_ext_header(temp.path(), header.clone(), &ext_header).unwrap();
        
        let data = vec![0u8; 32];
        writer.write_data(&data).unwrap();
        drop(writer);
        
        let reader = MrcReader::open(temp.path()).unwrap();
        assert_eq!(reader.ext_header().len(), 100);
    }
}

#[cfg(all(test, feature = "std"))]
mod volume_tests {
    use mrc::{Volume, Mode};
    
    #[test]
    fn test_volume_builder() {
        let volume: Volume<f32, _> = Volume::builder()
            .dimensions(64, 64, 64)
            .voxel_size(1.5, 1.5, 2.0)
            .origin(100.0, 100.0, 50.0)
            .build_allocated();
        
        assert_eq!(volume.dimensions(), (64, 64, 64));
        assert_eq!(volume.header().mode(), Mode::Float32);
        assert_eq!(volume.header().xorigin(), 100.0);
    }
    
    #[test]
    fn test_volume_access() {
        let mut volume: Volume<f32, _> = Volume::builder()
            .dimensions(4, 4, 4)
            .build_allocated();
        
        // Set a value
        volume.set_at(1, 2, 3, 42.0);
        
        // Get it back
        assert_eq!(volume.get_at(1, 2, 3), 42.0);
        
        // Bounds checking
        assert!(volume.get_at_opt(4, 0, 0).is_none());
        assert!(volume.get_at_checked(4, 0, 0).is_err());
    }
    
    #[test]
    fn test_slice_extraction() {
        let mut volume: Volume<f32, _> = Volume::builder()
            .dimensions(4, 4, 4)
            .build_allocated();
        
        // Fill with values
        for z in 0..4 {
            for y in 0..4 {
                for x in 0..4 {
                    volume.set_at(x, y, z, (x + y * 4 + z * 16) as f32);
                }
            }
        }
        
        // Extract slice
        let slice = volume.slice(2).unwrap();
        assert_eq!(slice.dimensions(), (4, 4));
        
        // Check value
        assert_eq!(slice.get(1, 2), (1 + 2 * 4 + 2 * 16) as f32);
    }
    
    #[test]
    fn test_subvolume() {
        let mut volume: Volume<f32, _> = Volume::builder()
            .dimensions(8, 8, 8)
            .build_allocated();
        
        // Fill with values
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    volume.set_at(x, y, z, (x + y * 8 + z * 64) as f32);
                }
            }
        }
        
        // Extract subvolume
        let subvol = volume.subvolume(2, 6, 2, 6, 2, 6).unwrap();
        assert_eq!(subvol.dimensions(), (4, 4, 4));
        
        // Check value at new coordinates
        assert_eq!(subvol.get_at(0, 0, 0), volume.get_at(2, 2, 2));
    }
    
    #[test]
    fn test_iteration() {
        let volume: Volume<i32, _> = Volume::builder()
            .dimensions(2, 2, 2)
            .build_allocated();
        
        let count = volume.iter().count();
        assert_eq!(count, 8);
        
        let coords: Vec<_> = volume.iter_coords().collect();
        assert_eq!(coords.len(), 8);
    }
}
