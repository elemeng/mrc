//! I/O tests for MRC library

#[cfg(all(test, feature = "std"))]
mod file_tests {
    use mrc::{Header, MrcReader, MrcWriter, Mode, Volume};
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_create_and_open() {
        let temp = NamedTempFile::new().unwrap();
        
        let header = Header::builder()
            .dimensions(32, 32, 32)
            .mode(Mode::Float32)
            .build();
        
        let writer = MrcWriter::create(temp.path(), header).unwrap();
        drop(writer);
        
        let reader = MrcReader::open(temp.path()).unwrap();
        assert_eq!(reader.header().dimensions(), (32, 32, 32));
    }
    
    #[test]
    fn test_write_and_read_data() {
        let temp = NamedTempFile::new().unwrap();
        
        let header = Header::builder()
            .dimensions(4, 4, 4)
            .mode(Mode::Int8)
            .build();
        
        let mut writer = MrcWriter::create(temp.path(), header.clone()).unwrap();
        let data: Vec<u8> = (0..64).collect();
        writer.write_data(&data).unwrap();
        drop(writer);
        
        let mut reader = MrcReader::open(temp.path()).unwrap();
        let read_data = reader.read_data().unwrap();
        assert_eq!(read_data.len(), 64);
        assert_eq!(read_data[0], 0);
        assert_eq!(read_data[63], 63);
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
    
    #[test]
    fn test_writer_builder() {
        let temp = NamedTempFile::new().unwrap();
        
        let data: Vec<u8> = (0..64).collect();
        
        MrcWriter::builder()
            .dimensions(4, 4, 4)
            .mode(Mode::Int8)
            .voxel_size(1.0, 1.0, 1.0)
            .origin(0.0, 0.0, 0.0)
            .data(data)
            .write(temp.path())
            .unwrap();
        
        let mut reader = MrcReader::open(temp.path()).unwrap();
        let read_data = reader.read_data().unwrap();
        assert_eq!(read_data.len(), 64);
    }
    
    #[test]
    fn test_typed_volume_io() {
        let temp = NamedTempFile::new().unwrap();
        
        // Create volume
        let mut volume: Volume<f32, _> = Volume::builder()
            .dimensions(8, 8, 8)
            .voxel_size(2.0, 2.0, 2.0)
            .build_allocated();
        
        // Set some values
        volume.set_at(0, 0, 0, 1.0);
        volume.set_at(7, 7, 7, 99.0);
        
        // Write volume
        let mut writer = MrcWriter::create(temp.path(), volume.header().clone()).unwrap();
        writer.write_data(volume.as_bytes()).unwrap();
        drop(writer);
        
        // Read back as typed volume
        let mut reader = MrcReader::open(temp.path()).unwrap();
        let read_volume: Volume<f32, _> = reader.read_volume().unwrap();
        
        assert_eq!(read_volume.dimensions(), (8, 8, 8));
        assert_eq!(read_volume.get_at(0, 0, 0), 1.0);
        assert_eq!(read_volume.get_at(7, 7, 7), 99.0);
    }
    
    #[test]
    fn test_dynamic_volume_io() {
        let temp = NamedTempFile::new().unwrap();
        
        let volume: Volume<i16, _> = Volume::builder()
            .dimensions(4, 4, 4)
            .mode(Mode::Int16)
            .build_allocated();
        
        let mut writer = MrcWriter::create(temp.path(), volume.header().clone()).unwrap();
        writer.write_data(volume.as_bytes()).unwrap();
        drop(writer);
        
        let mut reader = MrcReader::open(temp.path()).unwrap();
        let data = reader.read().unwrap();
        
        assert_eq!(data.mode(), Mode::Int16);
        assert!(data.as_i16().is_some());
        assert!(data.as_f32().is_none());
    }
}
