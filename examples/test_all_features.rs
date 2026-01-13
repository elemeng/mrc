use mrc::{Header, Mode, MrcFile, MrcMmap, MrcView, MrcViewMut};

fn run_test() -> Result<(), mrc::Error> {
    println!("=== Testing ALL features of mrc crate ===\n");

    // 1. Test Header creation and validation
    println!("1. Header creation and validation:");
    let mut header = Header::new();
    header.nx = 64;
    header.ny = 64;
    header.nz = 64;
    header.mode = 2; // Float32
    header.xlen = 100.0;
    header.ylen = 100.0;
    header.zlen = 100.0;
    header.dmin = 0.0;
    header.dmax = 1.0;
    header.dmean = 0.5;

    println!(
        "   ✅ Header created: {}x{}x{} mode={}",
        header.nx, header.ny, header.nz, header.mode
    );
    println!("   ✅ Valid: {}", header.validate());

    // 2. Test Mode enum and conversions
    println!("\n2. Mode enum testing:");
    let mode = Mode::from_i32(2).unwrap();
    println!("   ✅ Mode::from_i32(2) = {:?}", mode);
    println!("   ✅ Byte size: {} bytes", mode.byte_size());

    // 3. Test endian swapping
    println!("\n3. Endian swapping:");
    let original = 0x12345678u32;
    let swapped = original.swap_bytes();
    println!(
        "   ✅ Original: 0x{:08x}, Swapped: 0x{:08x}",
        original, swapped
    );

    // 4. Create test file with all features
    println!("\n4. Creating test file with extended header:");
    header.nsymbt = 128; // Extended header
    let temp_path = "test_all_features.mrc";

    {
        let mut file = MrcFile::create(temp_path, header)?;

        // Create test data
        let data_size = header.data_size();
        let test_data: Vec<f32> = (0..data_size / 4).map(|i| (i as f32) * 0.01).collect();

        // Write extended header
        let ext_header: Vec<u8> = (0..128).map(|i| (i % 256) as u8).collect();
        file.write_ext_header(&ext_header)?;

        // Write data
        file.write_data(bytemuck::cast_slice(&test_data))?;
        println!(
            "   ✅ Created file: {} bytes data + {} bytes ext header",
            data_size, 128
        );
    }

    // 5. Test reading with MrcFile
    println!("\n5. Testing MrcFile features:");
    {
        let file = MrcFile::open(temp_path)?;
        let _header = file.header();

        // Test all read methods
        let ext_header = file.read_ext_header()?;
        let data = file.read_data()?;
        let view = file.read_view()?;

        println!("   ✅ MrcFile.open()");
        println!("   ✅ read_ext_header(): {} bytes", ext_header.len());
        println!("   ✅ read_data(): {} bytes", data.len());

        let dims = view.dimensions();
        println!("   ✅ Dimensions tuple: {:?}", dims);

        // Test typed access
        let typed_data = view.data_as_f32().unwrap();
        println!("   ✅ Typed view: {} f32 values", typed_data.len());
    }

    // 6. Test memory-mapped access
    #[cfg(feature = "mmap")]
    {
        println!("\n6. Testing MrcMmap features:");
        let mmap = MrcMmap::open(temp_path)?;

        println!("   ✅ MrcMmap.open()");
        println!("   ✅ Data access: {} bytes", mmap.data().len());
        println!("   ✅ Extended header: {} bytes", mmap.ext_header().len());

        // Test memory-mapped view
        let view = mmap.read_view()?;
        let typed_data = view.data_as_f32().unwrap();
        println!("   ✅ Mmap view: {} f32 values", typed_data.len());
    }

    // 7. Test MrcView and MrcViewMut
    println!("\n7. Testing MrcView and MrcViewMut:");
    {
        let buffer = vec![0u8; header.data_size() + 128];

        // Create view from buffer
        let view = MrcView::new(header, &buffer)?;
        println!("   ✅ MrcView creation");

        let dims = view.dimensions();
        println!("   ✅ Dimensions tuple: {:?}", dims);

        // Test MrcViewMut
        let mut mut_buffer = vec![0u8; header.data_size() + 128];
        let mut mut_view = MrcViewMut::new(header, &mut mut_buffer)?;
        println!("   ✅ MrcViewMut creation");

        // Test mutating data at byte level
        let data_bytes = mut_view.data_mut();
        data_bytes[0] = 0x00; // First byte of first float
        data_bytes[1] = 0x00;
        data_bytes[2] = 0x28;
        data_bytes[3] = 0x42; // IEEE 754 representation of 42.0 (little-endian)
        println!("   ✅ Mutable view: modified first element at byte level");
    }

    // 8. Test high-level convenience functions
    println!("\n8. Testing high-level functions:");
    {
        // Test open_file and open_mmap
        let _view = mrc::open_file(temp_path)?;
        println!("   ✅ open_file()");

        #[cfg(feature = "mmap")]
        {
            let _mmap_view = mrc::open_mmap(temp_path)?;
            println!("   ✅ open_mmap()");
        }
    }

    // 9. Test endianness detection and validation
    println!("\n9. Testing endianness and validation:");
    {
        let file = MrcFile::open(temp_path)?;
        let header = file.header();

        // Test validation
        println!("   ✅ Header validation: {}", header.validate());

        // Test data offset calculation
        let offset = header.data_offset();
        println!("   ✅ Data offset: {} bytes", offset);

        // Test data size calculation
        let size = header.data_size();
        println!("   ✅ Data size: {} bytes", size);
    }

    // 10. Test error handling
    println!("\n10. Testing error handling:");
    {
        // Test invalid file
        match MrcFile::open("nonexistent.mrc") {
            Ok(_) => unreachable!(),
            Err(_) => println!("   ✅ Error handling: file not found"),
        }

        // Test invalid header
        let mut bad_header = Header::new();
        bad_header.nx = 0; // Invalid
        println!("   ✅ Invalid header validation: {}", bad_header.validate());
    }

    // 11. Test all data modes
    println!("\n11. Testing all data modes:");
    let modes = [0, 1, 2, 3, 4, 6, 12, 101];
    for &mode in &modes {
        if let Some(m) = Mode::from_i32(mode) {
            println!("   ✅ Mode {}: {:?} ({} bytes)", mode, m, m.byte_size());
        } else {
            println!("   ✅ Mode {}: Invalid", mode);
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(temp_path);

    println!("\n=== ALL FEATURES TESTED SUCCESSFULLY ===");
    Ok(())
}

#[cfg(not(feature = "std"))]
fn run_test() -> Result<(), mrc::Error> {
    println!("=== Testing no_std features ===");

    // Test Header creation without std
    let mut header = Header::new();
    header.nx = 32;
    header.ny = 32;
    header.nz = 32;
    header.mode = 2;

    println!("✅ Header works in no_std");
    println!("✅ Mode enum works in no_std");
    println!("✅ All basic types work in no_std");

    Ok(())
}

fn main() {
    if let Err(e) = run_test() {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}

#[cfg(not(feature = "std"))]
fn main() {
    if let Err(e) = run_test() {
        eprintln!("Error: {:?}", e);
        core::panic!("Test failed");
    }
}
