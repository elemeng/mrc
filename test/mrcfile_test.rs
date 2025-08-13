#[cfg(test)]
#[cfg(feature = "std")]
mod backend_tests {

    use crate::mrcfile::{MrcFile, MrcMmap};
    use crate::{Header, Mode};
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
            let mmap_map = MrcMmap::open(temp_file.path())
                .unwrap()
                .read_view()
                .unwrap();

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
        let map = MrcFile::open(temp_file.path())
            .unwrap()
            .read_view()
            .unwrap();

        // Verify
        let read_data: &[f32] = map.view().unwrap();
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
        let map = MrcFile::open(temp_file.path())
            .unwrap()
            .read_view()
            .unwrap();

        assert_eq!(map.ext_header(), &ext_data[..]);
        assert_eq!(map.data().len(), 16);
    }
}
