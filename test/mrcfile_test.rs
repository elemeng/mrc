#[cfg(test)]
#[cfg(feature = "std")]
mod backend_tests {

    use crate::mrcfile::{MrcFile, MrcMmap};
    use crate::{Header, Mode, MrcView};
    use alloc::vec;
    use core::f32::consts::PI;
    use tempfile::NamedTempFile;

    #[test]
    fn test_backends_identical_view() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 4;
        header.ny = 4;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 100; // Test with extended header

        let original_data = vec![1.0f32; 4 * 4 * 2];
        let ext_header_data = vec![0xAAu8; 100];

        // Validate header before creation
        assert!(
            header.validate(),
            "Header validation failed: nx={}, ny={}, nz={}, mode={}",
            header.nx,
            header.ny,
            header.nz,
            header.mode
        );

        let data_size = header.data_size();
        assert!(
            data_size > 0,
            "Data size calculation failed: {}x{}x{} mode={} = {} bytes",
            header.nx,
            header.ny,
            header.nz,
            header.mode,
            data_size
        );

        // Create test file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_ext_header(&ext_header_data).unwrap();
            file.write_data(bytemuck::cast_slice(&original_data))
                .unwrap();
        }

        // Test both backends
        let backend = MrcFile::open(temp_file.path()).unwrap();
        let io_map = backend.read_view().unwrap();

        #[cfg(feature = "mmap")]
        {
            let mmap = MrcMmap::open(temp_file.path()).unwrap();
            let mmap_map = mmap.read_view().unwrap();

            // Verify identical headers
            assert_eq!(io_map.header(), mmap_map.header());

            // Verify identical data
            assert_eq!(io_map.data(), mmap_map.data());

            // Verify identical extended headers
            assert_eq!(io_map.ext_header(), mmap_map.ext_header());
        }

        // Test basic functionality
        let (w, h, d) = io_map.dimensions();
        assert_eq!((w, h, d), (4, 4, 2));
        assert_eq!(io_map.mode(), Some(Mode::Float32));
        assert_eq!(io_map.ext_header().len(), 100);
    }

    #[test]
    fn test_round_trip_save() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 3;
        header.ny = 3;
        header.nz = 3;
        header.mode = 2;
        header.nsymbt = 0;

        let original_data = vec![PI; 3 * 3 * 3];

        // Create and write
        {
            let mut backend = MrcFile::create(temp_file.path(), header).unwrap();
            backend
                .write_data(bytemuck::cast_slice(&original_data))
                .unwrap();
        }

        // Read back
        let file = MrcFile::open(temp_file.path()).unwrap();
        let map = file.read_view().unwrap();

        // Verify
        let read_data = map.data_as_f32().unwrap();
        assert_eq!(read_data, original_data);
        assert_eq!(map.header().nx, 3);
        assert_eq!(map.header().ny, 3);
        assert_eq!(map.header().nz, 3);
    }

    #[test]
    fn test_ext_header_read_write() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 1;
        header.mode = 2;
        header.nsymbt = 128; // 128-byte extended header

        let ext_data = vec![0x42u8; 128];
        let data = vec![1.0f32; 4];

        // Create file with extended header
        {
            let mut backend = MrcFile::create(temp_file.path(), header).unwrap();
            backend.write_ext_header(&ext_data).unwrap();
            backend.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Read back
        let file = MrcFile::open(temp_file.path()).unwrap();
        let map = file.read_view().unwrap();

        assert_eq!(map.ext_header(), &ext_data[..]);
        assert_eq!(map.data().len(), 16);
    }

    #[test]
    fn test_file_open_errors() {
        // Test opening non-existent file
        let result = MrcFile::open("/nonexistent/path/file.mrc");
        assert!(result.is_err());

        // Test creating file in non-existent directory
        let header = Header::new();
        let result = MrcFile::create("/nonexistent/path/file.mrc", header);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_invalid_header_errors() {
        let temp_file = NamedTempFile::new().unwrap();

        // Test invalid header validation
        let mut header = Header::new();
        header.nx = 0;
        header.ny = 0;
        header.nz = 0;
        header.mode = 2;

        let result = MrcFile::create(temp_file.path(), header);
        assert!(matches!(result, Err(crate::Error::InvalidHeader)));

        // Test invalid mode
        let mut header = Header::new();
        header.nx = 10;
        header.ny = 10;
        header.nz = 10;
        header.mode = 99; // Invalid mode

        let result = MrcFile::create(temp_file.path(), header);
        assert!(matches!(result, Err(crate::Error::InvalidHeader)));
    }

    #[test]
    fn test_file_write_errors() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 4;
        header.ny = 4;
        header.nz = 4;
        header.mode = 2;

        // Create valid file
        let mut file = MrcFile::create(temp_file.path(), header).unwrap();

        // Test writing wrong data size
        let wrong_data = vec![0u8; 100]; // Need 4*4*4*4 = 256 bytes
        let result = file.write_data(&wrong_data);
        assert!(matches!(result, Err(crate::Error::InvalidDimensions)));

        // Test writing correct data size
        let correct_data = vec![0u8; 256];
        let result = file.write_data(&correct_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_ext_header_write_errors() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 64; // 64-byte extended header

        let mut file = MrcFile::create(temp_file.path(), header).unwrap();

        // Test writing wrong extended header size
        let wrong_ext_data = vec![0u8; 32]; // Need 64 bytes
        let result = file.write_ext_header(&wrong_ext_data);
        assert!(matches!(result, Err(crate::Error::InvalidDimensions)));

        // Test writing correct extended header size
        let correct_ext_data = vec![0u8; 64];
        let result = file.write_ext_header(&correct_ext_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_header_access() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 8;
        header.ny = 8;
        header.nz = 8;
        header.mode = 1; // Int16

        let file = MrcFile::create(temp_file.path(), header).unwrap();

        // Test header access
        let read_header = file.header();
        assert_eq!(read_header.nx, 8);
        assert_eq!(read_header.ny, 8);
        assert_eq!(read_header.nz, 8);
        assert_eq!(read_header.mode, 1);
    }

    #[test]
    fn test_file_read_write_all_modes() {
        let modes = [
            (Mode::Int8, 0),
            (Mode::Int16, 1),
            (Mode::Float32, 2),
            (Mode::Int16Complex, 3),
            (Mode::Float32Complex, 4),
            (Mode::Uint16, 6),
            (Mode::Float16, 12),
        ];

        for (mode, mode_id) in modes {
            let temp_file = NamedTempFile::new().unwrap();

            let mut header = Header::new();
            header.nx = 3;
            header.ny = 3;
            header.nz = 3;
            header.mode = mode_id;

            let data_size = header.data_size();
            let data = vec![0u8; data_size];

            // Write
            {
                let mut file = MrcFile::create(temp_file.path(), header).unwrap();
                file.write_data(&data).unwrap();
            }

            // Read back
            let file = MrcFile::open(temp_file.path()).unwrap();
            let view = file.read_view().unwrap();

            assert_eq!(view.mode(), Some(mode));
            assert_eq!(view.data().len(), data_size);
        }
    }

    #[test]
    fn test_file_with_large_ext_header() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 1;
        header.ny = 1;
        header.nz = 1;
        header.mode = 2;
        header.nsymbt = 1024; // 1KB extended header

        let ext_data = vec![0xAAu8; 1024];
        let data = vec![42.0f32; 1];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_ext_header(&ext_data).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Read back
        let file = MrcFile::open(temp_file.path()).unwrap();
        let view = file.read_view().unwrap();

        assert_eq!(view.ext_header().len(), 1024);
        assert_eq!(view.ext_header(), ext_data);
        assert_eq!(view.data().len(), 4); // 1 float = 4 bytes

        let float_data = view.data_as_f32().unwrap();
        assert_eq!(float_data[0], 42.0);
    }

    #[test]
    fn test_file_zero_ext_header() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 0; // No extended header

        let data = vec![1.0f32; 8];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Read back
        let file = MrcFile::open(temp_file.path()).unwrap();
        let view = file.read_view().unwrap();

        assert_eq!(view.ext_header().len(), 0);
        assert_eq!(view.data().len(), 32); // 8 floats * 4 bytes
    }

    #[test]
    fn test_mrcfile_header_access() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 4;
        header.ny = 4;
        header.nz = 4;
        header.mode = 1; // Int16

        let mut file = MrcFile::create(temp_file.path(), header).unwrap();

        // Test header access
        let read_header = file.header();
        assert_eq!(read_header.nx, 4);
        assert_eq!(read_header.ny, 4);
        assert_eq!(read_header.nz, 4);
        assert_eq!(read_header.mode, 1);

        // Test header_mut access
        let mut_header = file.header_mut();
        mut_header.nx = 8;
        assert_eq!(mut_header.nx, 8);
        assert_eq!(file.header().nx, 8);
    }

    #[test]
    fn test_mrcfile_read_ext_header() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 64; // 64-byte extended header

        let ext_data = vec![0xAAu8; 64];
        let data = vec![1.0f32; 8];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_ext_header(&ext_data).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Test read_ext_header
        let file = MrcFile::open(temp_file.path()).unwrap();
        let ext_header = file.read_ext_header().unwrap();
        assert_eq!(ext_header.len(), 64);
        assert_eq!(ext_header, ext_data);
    }

    #[test]
    fn test_mrcfile_read_data() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 0;

        let data = vec![
            1.0f32, 2.0f32, 3.0f32, 4.0f32, 5.0f32, 6.0f32, 7.0f32, 8.0f32,
        ];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Test read_data
        let file = MrcFile::open(temp_file.path()).unwrap();
        let read_data = file.read_data().unwrap();
        assert_eq!(read_data.len(), 32); // 8 floats * 4 bytes

        let floats: &[f32] = bytemuck::cast_slice(read_data);
        assert_eq!(floats, data);
    }

    #[test]
    fn test_mrcfile_write_view() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 32; // 32-byte extended header

        let ext_data = vec![0xBBu8; 32];
        let data = vec![
            1.0f32, 2.0f32, 3.0f32, 4.0f32, 5.0f32, 6.0f32, 7.0f32, 8.0f32,
        ];
        let full_data = [&ext_data[..], bytemuck::cast_slice(&data)].concat();

        // Create view
        let view = MrcView::new(header.clone(), &full_data).unwrap();

        // Test write_view
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_view(&view).unwrap();
        }

        // Verify by reading back
        let file = MrcFile::open(temp_file.path()).unwrap();
        let read_view = file.read_view().unwrap();
        assert_eq!(read_view.ext_header(), ext_data);

        let floats = read_view.data_as_f32().unwrap();
        assert_eq!(floats, data);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mmap_header_access() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 3;
        header.ny = 3;
        header.nz = 3;
        header.mode = 2;

        let data = vec![1.0f32; 27];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Test mmap header access
        let mmap = MrcMmap::open(temp_file.path()).unwrap();
        assert_eq!(mmap.header().nx, 3);
        assert_eq!(mmap.header().ny, 3);
        assert_eq!(mmap.header().nz, 3);
        assert_eq!(mmap.header().mode, 2);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mmap_ext_header_access() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 48; // 48-byte extended header

        let ext_data = vec![0xCCu8; 48];
        let data = vec![1.0f32; 8];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_ext_header(&ext_data).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Test mmap extended header access
        let mmap = MrcMmap::open(temp_file.path()).unwrap();
        assert_eq!(mmap.ext_header().len(), 48);
        assert_eq!(mmap.ext_header(), ext_data);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mmap_data_access() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 2;
        header.ny = 2;
        header.nz = 2;
        header.mode = 2;
        header.nsymbt = 0;

        let data = vec![
            1.1f32, 2.2f32, 3.3f32, 4.4f32, 5.5f32, 6.6f32, 7.7f32, 8.8f32,
        ];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Test mmap data access
        let mmap = MrcMmap::open(temp_file.path()).unwrap();
        assert_eq!(mmap.data().len(), 32); // 8 floats * 4 bytes
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mmap_basic_functionality() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut header = Header::new();
        header.nx = 3;
        header.ny = 3;
        header.nz = 3;
        header.mode = 2;

        let data = vec![1.0f32; 3 * 3 * 3];

        // Create file
        {
            let mut file = MrcFile::create(temp_file.path(), header).unwrap();
            file.write_data(bytemuck::cast_slice(&data)).unwrap();
        }

        // Test mmap
        let mmap = MrcMmap::open(temp_file.path()).unwrap();
        let view = mmap.read_view().unwrap();

        assert_eq!(view.dimensions(), (3, 3, 3));
        assert_eq!(view.mode(), Some(Mode::Float32));
    }
}
