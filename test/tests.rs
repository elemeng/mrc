#[cfg(test)]
mod header_tests {
    use crate::{Header, Mode, MrcView};
    use alloc::vec;
    use core::mem;

    #[test]
    fn test_header_size() {
        assert_eq!(mem::size_of::<Header>(), 1024);
    }

    #[test]
    fn test_header_alignment() {
        assert_eq!(mem::align_of::<Header>(), 4);
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
        assert_eq!(Mode::Int16Complex.byte_size(), 2);
        assert_eq!(Mode::Float32Complex.byte_size(), 4);
        assert_eq!(Mode::Uint16.byte_size(), 1);
        assert_eq!(Mode::Float16.byte_size(), 2);
    }

    #[test]
    fn test_mode_properties() {
        assert!(Mode::Int16Complex.is_complex());
        assert!(Mode::Float32Complex.is_complex());
        assert!(!Mode::Int8.is_complex());
        assert!(!Mode::Float32.is_complex());

        assert!(Mode::Int8.is_integer());
        assert!(Mode::Int16.is_integer());
        assert!(Mode::Int16Complex.is_integer());
        assert!(Mode::Uint16.is_integer());
        assert!(!Mode::Float32.is_integer());
        assert!(!Mode::Float16.is_integer());

        assert!(Mode::Float32.is_float());
        assert!(Mode::Float32Complex.is_float());
        assert!(Mode::Float16.is_float());
        assert!(!Mode::Int8.is_float());
        assert!(!Mode::Int16.is_float());
    }

    #[test]
    fn test_header_data_size() {
        let mut header = Header::new();
        header.nx = 10;
        header.ny = 20;
        header.nz = 30;

        header.mode = 0;
        assert_eq!(header.data_size(), (10 * 20 * 30));

        header.mode = 1;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 2);

        header.mode = 2;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 4);

        header.mode = 3;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 2);

        header.mode = 4;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 4);

        header.mode = 6;
        assert_eq!(header.data_size(), (10 * 20 * 30));

        header.mode = 12;
        assert_eq!(header.data_size(), 10 * 20 * 30 * 2);

        header.mode = 5; // Invalid mode
        assert_eq!(header.data_size(), 0);

        header.mode = 5;
        assert_eq!(header.data_size(), 0);
    }

    #[test]
    fn test_header_data_offset() {
        let mut header = Header::new();

        header.nsymbt = 0;
        assert_eq!(header.data_offset(), 1024);

        header.nsymbt = 100;
        assert_eq!(header.data_offset(), 1124);
    }

    #[test]
    fn test_header_exttyp_and_nversion() {
        let mut header = Header::new();

        // Test EXTTYP as 4-byte integer
        assert_eq!(header.exttyp(), 0);

        // Test setting EXTTYP as 4-char string
        header.set_exttyp_str("CCP4").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "CCP4");
        assert_eq!(
            header.exttyp(),
            i32::from_le_bytes([b'C', b'C', b'P', b'4'])
        );

        // Test other valid formats
        header.set_exttyp_str("MRCO").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "MRCO");

        header.set_exttyp_str("SERI").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "SERI");

        header.set_exttyp_str("AGAR").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "AGAR");

        header.set_exttyp_str("FEI1").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "FEI1");

        header.set_exttyp_str("FEI2").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "FEI2");

        header.set_exttyp_str("HDF5").unwrap();
        assert_eq!(header.exttyp_str().unwrap(), "HDF5");

        // Test NVERSION with latest format (20141)
        assert_eq!(header.nversion(), 0);
        header.set_nversion(20141);
        assert_eq!(header.nversion(), 20141);

        // Test other version formats
        header.set_nversion(20140); // 2014.0
        assert_eq!(header.nversion(), 20140);

        // Test invalid string length for EXTTYP
        assert!(header.set_exttyp_str("CCP").is_err());
        assert!(header.set_exttyp_str("CCP4X").is_err());
    }

    #[test]
    fn test_header_endian_swap_includes_exttyp_and_nversion() {
        let mut header = Header::new();

        // Test simple integer values first
        header.set_exttyp(42);
        header.set_nversion(20141);

        let original_exttyp = header.exttyp();
        let original_nversion = header.nversion();

        assert_eq!(original_exttyp, 42);
        assert_eq!(original_nversion, 20141);

        header.swap_endian();

        // Both should be swapped
        assert_eq!(header.exttyp(), 42_i32.swap_bytes());
        assert_eq!(header.nversion(), 20141_i32.swap_bytes());

        // Swap back to verify
        header.swap_endian();
        assert_eq!(header.exttyp(), original_exttyp);
        assert_eq!(header.nversion(), original_nversion);
    }

    #[test]
    fn test_header_field_positions() {
        // Verify byte positions match MRC 2014 specification
        let header = Header::new();

        // Calculate byte positions
        let base_ptr = &header as *const Header as *const u8;

        // Check that EXTRA starts at byte 97
        let extra_ptr = &header.extra as *const [u8; 100] as *const u8;
        assert_eq!(extra_ptr as usize - base_ptr as usize, 96);

        // Check that ORIGIN starts at byte 197
        let origin_ptr = &header.origin as *const [f32; 3] as *const u8;
        assert_eq!(origin_ptr as usize - base_ptr as usize, 196);

        // Check that MAP starts at byte 209
        let map_ptr = &header.map as *const [u8; 4] as *const u8;
        assert_eq!(map_ptr as usize - base_ptr as usize, 208);

        // Check that LABEL starts at byte 225
        let label_ptr = &header.label as *const [u8; 800] as *const u8;
        assert_eq!(label_ptr as usize - base_ptr as usize, 224);
    }

    #[test]
    fn test_header_validation() {
        let mut header = Header::new();
        assert!(!header.validate());

        header.nx = 10;
        header.ny = 20;
        header.nz = 30;
        header.mode = 2;
        assert!(header.validate());

        header.nx = 0;
        assert!(!header.validate());

        header.nx = 10;
        header.mode = 5;
        assert!(!header.validate());

        header.mode = -1;
        assert!(!header.validate());
    }

    #[test]
    fn test_map_all_modes() {
        let dimensions = [(2, 2, 2), (4, 4, 4), (8, 8, 8)];
        let modes = [
            (Mode::Int8, 0),
            (Mode::Int16, 1),
            (Mode::Float32, 2),
            (Mode::Int16Complex, 3),
            (Mode::Float32Complex, 4),
            (Mode::Uint16, 6),
            (Mode::Float16, 12),
        ];

        for (nx, ny, nz) in dimensions {
            for (mode, mode_id) in modes {
                let mut header = Header::new();
                header.nx = nx as i32;
                header.ny = ny as i32;
                header.nz = nz as i32;
                header.mode = mode_id;

                let data_size = header.data_size();
                let data = vec![0u8; data_size];

                let map = MrcView::new(header, &data);
                assert!(
                    map.is_ok(),
                    "Failed for mode {mode:?} with dimensions {nx}x{ny}x{nz}"
                );

                let map = map.unwrap();
                assert_eq!(map.dimensions(), (nx, ny, nz));
                assert_eq!(map.mode(), Some(mode));
                assert_eq!(map.data().len(), data_size);
            }
        }
    }

    #[test]
    fn test_map_view_types() {
        let mut header = Header::new();
        header.nx = 4;
        header.ny = 4;
        header.nz = 4;

        // Test i8
        header.mode = 0;
        let data = vec![0i8; 64];
        let map = MrcView::new(header, bytemuck::cast_slice(&data)).unwrap();
        let view: &[i8] = map.view().unwrap();
        assert_eq!(view.len(), 64);

        // Test i16
        header.mode = 1;
        let data = vec![0i16; 64];
        let map = MrcView::new(header, bytemuck::cast_slice(&data)).unwrap();
        let view: &[i16] = map.view().unwrap();
        assert_eq!(view.len(), 64);

        // Test f32
        header.mode = 2;
        let data = vec![0f32; 64];
        let map = MrcView::new(header, bytemuck::cast_slice(&data)).unwrap();
        let view: &[f32] = map.view().unwrap();
        assert_eq!(view.len(), 64);

        // Test f16 (mode 12)
        header.mode = 12;
        let data = vec![0u16; 64]; // f16 backed by u16
        let map = MrcView::new(header, bytemuck::cast_slice(&data)).unwrap();
        let view: &[u16] = map.view().unwrap();
        assert_eq!(view.len(), 64);
    }

    #[test]
    fn test_map_slice_bytes() {
        let mut header = Header::new();
        header.nx = 10;
        header.ny = 10;
        header.nz = 10;
        header.mode = 2;

        let data = vec![0u8; 4000];
        let map = MrcView::new(header, &data).unwrap();

        let slice = map.slice_bytes(0..100).unwrap();
        assert_eq!(slice.len(), 100);

        let slice = map.slice_bytes(100..200).unwrap();
        assert_eq!(slice.len(), 100);

        assert!(map.slice_bytes(0..5000).is_err());
        assert!(map.slice_bytes(4000..4100).is_err());
    }

    #[test]
    fn test_doc_example_64x64x64_f32() {
        let mut header = Header::new();
        header.nx = 64;
        header.ny = 64;
        header.nz = 64;
        header.mode = 2;

        let data_size = header.data_size();
        assert_eq!(data_size, 64 * 64 * 64 * 4);

        let data = vec![0f32; 64 * 64 * 64];
        let map = MrcView::new(header, bytemuck::cast_slice(&data)).unwrap();

        let volume: &[f32] = map.view().unwrap();
        assert_eq!(volume.len(), 64 * 64 * 64);
    }
}
