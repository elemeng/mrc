//! I/O tests for MRC library

#[cfg(all(test, feature = "std"))]
mod file_tests {
    use mrc::{Header, MrcReader, MrcWriter, Mode, RawHeader};
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_create_and_open() {
        let temp = NamedTempFile::new().unwrap();
        
        // Create header
        let mut header = Header::default();
        header.nx = 32;
        header.ny = 32;
        header.nz = 32;
        header.mode = Mode::Float32;
        
        // Create file
        let writer = MrcWriter::create(temp.path(), header.clone()).unwrap();
        drop(writer);
        
        // Open and verify
        let reader = MrcReader::open(temp.path()).unwrap();
        assert_eq!(reader.header().dimensions(), (32, 32, 32));
    }
    
    #[test]
    fn test_write_and_read_data() {
        let temp = NamedTempFile::new().unwrap();
        
        // Create header
        let mut header = Header::default();
        header.nx = 4;
        header.ny = 4;
        header.nz = 4;
        header.mode = Mode::Int8;
        
        // Create file with data
        let mut writer = MrcWriter::create(temp.path(), header.clone()).unwrap();
        let data: Vec<u8> = (0..64).collect();
        writer.write_data(&data).unwrap();
        drop(writer);
        
        // Read back
        let mut reader = MrcReader::open(temp.path()).unwrap();
        let read_data = reader.read_data().unwrap();
        assert_eq!(read_data.len(), 64);
        assert_eq!(read_data[0], 0);
        assert_eq!(read_data[63], 63);
    }
    
    #[test]
    fn test_ext_header() {
        let temp = NamedTempFile::new().unwrap();
        
        // Create header with ext header size
        let mut header = Header::default();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = Mode::Float32;
        header.nsymbt = 100;
        
        // Create file with ext header
        let ext_header = vec![0xABu8; 100];
        let mut writer = MrcWriter::create_with_ext_header(temp.path(), header.clone(), &ext_header).unwrap();
        
        // Write data
        let data = vec![0u8; 32]; // 2*2*2*4 bytes
        writer.write_data(&data).unwrap();
        drop(writer);
        
        // Read back
        let reader = MrcReader::open(temp.path()).unwrap();
        assert_eq!(reader.ext_header().len(), 100);
    }
}