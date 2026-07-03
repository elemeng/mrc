use mrc::*;

/// Create a unique temp file path that is automatically deleted on drop.
struct TempMrc(std::path::PathBuf);

impl TempMrc {
    fn new(suffix: &str) -> Self {
        let mut p = std::env::temp_dir();
        p.push(format!("mrc_test_{}_{}.mrc", std::process::id(), suffix));
        // Remove any leftover from a previous run
        let _ = std::fs::remove_file(&p);
        Self(p)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempMrc {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Write Float32 volume, read it back byte-for-byte.
#[test]
fn roundtrip_f32() {
    let f = TempMrc::new("f32");
    let nx = 16;
    let ny = 8;
    let nz = 4;

    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<f32>().unwrap();
    assert_eq!(block.data, data);
    assert_eq!(&block.offset, &[0, 0, 0]);
    assert_eq!(&block.shape, &[nx, ny, nz]);

    // convert_volume::<f32> on Float32 should give the same result
    let block2 = r.convert::<f32>().read_volume().unwrap();
    assert_eq!(block2.data, data);
}

/// Write Int16, read back via convert_volume::<f32> (auto-conversion).
#[test]
fn roundtrip_i16_to_f32() {
    let f = TempMrc::new("i16");
    let nx = 16;
    let ny = 8;
    let nz = 4;
    let total = nx * ny * nz;

    let src: Vec<i16> = (0..total).map(|i| (i as i16) - 100).collect();
    let expected_f32: Vec<f32> = src.iter().map(|&v| v as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    // convert_volume::<f32> auto-converts Int16 → f32
    let block = r.convert::<f32>().read_volume().unwrap();
    assert_eq!(block.data, expected_f32);

    // as::<f32>().slices() should also match
    let all: Vec<f32> = r
        .convert::<f32>()
        .slices()
        .flat_map(|s| s.unwrap().data)
        .collect();
    assert_eq!(all, expected_f32);
}

/// Write Uint16, read back via read_volume::<u16>().
#[test]
fn roundtrip_u16() {
    let f = TempMrc::new("u16");
    let nx = 8;
    let ny = 8;
    let nz = 2;

    let src: Vec<u16> = (0..nx * ny * nz).map(|i| (i * 2) as u16).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<u16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<u16>().unwrap();
    assert_eq!(block.data, src);
}

/// Write multiple slabs, read back with subregion.
#[test]
fn roundtrip_subregion() {
    let f = TempMrc::new("subregion");
    let nx = 32;
    let ny = 32;
    let nz = 8;

    let mut w = create(f.path())
        .shape([nx, ny, nz])
        .mode::<f32>()
        .finish()
        .unwrap();
    for z in 0..nz {
        let slice = vec![(z * nx * ny) as f32; nx * ny];
        w.write_block(&VoxelBlock::new([0, 0, z], [nx, ny, 1], slice).unwrap())
            .unwrap();
    }
    w.finalize().unwrap();

    let r = Reader::open(f.path()).unwrap();
    // Read a middle subregion
    let block = r.subregion::<f32>([4, 4, 2], [8, 8, 3]).unwrap();
    assert_eq!(block.offset, [4, 4, 2]);
    assert_eq!(block.shape, [8, 8, 3]);
    assert_eq!(block.data.len(), 8 * 8 * 3);
}

/// Read entire volume via read_volume matches collecting as::<f32>().slices().
#[test]
fn read_volume_via_convert_slices_f32() {
    let f = TempMrc::new("vol_vs_slices");
    let nx = 10;
    let ny = 10;
    let nz = 5;

    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let vol = r.read_volume::<f32>().unwrap();
    let collected: Vec<f32> = r
        .convert::<f32>()
        .slices()
        .flat_map(|s| s.unwrap().data)
        .collect();
    assert_eq!(vol.data, collected);
}

/// Gzip compressed roundtrip.
#[cfg(feature = "gzip")]
#[test]
fn roundtrip_gzip() {
    let f = TempMrc::new("gzip");
    let nx = 8;
    let ny = 8;
    let nz = 4;

    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish_gzip()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    // Reader::open auto-detects gzip
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<f32>().unwrap();
    assert_eq!(block.data, data);
}

/// Header statistics roundtrip: write data, update stats, read back.
#[test]
fn update_header_stats_roundtrip() {
    let f = TempMrc::new("stats");
    let nx = 4;
    let ny = 4;
    let nz = 2;
    let total = nx * ny * nz;

    let data: Vec<f32> = (0..total).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    assert!(r.validate_header_stats().is_ok());
}

// ── Packed4Bit (Mode 101) tests ──────────────────────────────────────

/// Write Mode 101 with even width, read back via read_volume_u8.
#[test]
fn mode101_roundtrip_even() {
    let f = TempMrc::new("m101_even");
    let nx = 4;
    let ny = 4;
    let nz = 2;
    let total = nx * ny * nz;

    let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode_raw(101)
            .finish()
            .unwrap();
        w.write_u4_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume_u8().unwrap();
    assert_eq!(block.data, src);

    // slices_u8 should also match
    let collected: Vec<u8> = r.slices_u8().flat_map(|s| s.unwrap().data).collect();
    assert_eq!(collected, src);
}

/// Write Mode 101 with odd width (nx=3), read back.
#[test]
fn mode101_roundtrip_odd() {
    let f = TempMrc::new("m101_odd");
    let nx = 3;
    let ny = 2;
    let nz = 1;
    let total = nx * ny * nz;

    let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode_raw(101)
            .finish()
            .unwrap();
        w.write_u4_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume_u8().unwrap();
    assert_eq!(block.data, src);
}

/// Header stats on a Mode 101 file.
#[test]
fn mode101_header_stats() {
    let f = TempMrc::new("m101_stats");
    let nx = 8;
    let ny = 4;
    let nz = 1;
    let total = nx * ny * nz;

    let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode_raw(101)
            .finish()
            .unwrap();
        w.write_u4_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
            .unwrap();
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    assert!(r.validate_header_stats().is_ok());
}

/// Value > 15 should produce an error.
#[test]
fn mode101_value_overflow() {
    let f = TempMrc::new("m101_overflow");
    let src = vec![16u8]; // 16 > 15
    let mut w = create(f.path())
        .shape([1, 1, 1])
        .mode_raw(101)
        .finish()
        .unwrap();
    let result = w.write_u4_block(&VoxelBlock::new([0, 0, 0], [1, 1, 1], src).unwrap());
    assert!(result.is_err());
}

// ── Complex mode roundtrip tests ──────────────────────────────────────

/// Write Int16Complex (Mode 3), read back.
#[test]
fn roundtrip_complex_i16() {
    let f = TempMrc::new("cpx_i16");
    let nx = 4;
    let ny = 4;
    let nz = 2;
    let total = nx * ny * nz;

    let src: Vec<Int16Complex> = (0..total)
        .map(|i| Int16Complex {
            real: i as i16,
            imag: (i * 10) as i16,
        })
        .collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<Int16Complex>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<Int16Complex>().unwrap();
    assert_eq!(block.data.len(), total);
    for (a, b) in block.data.iter().zip(src.iter()) {
        assert_eq!(a.real, b.real);
        assert_eq!(a.imag, b.imag);
    }

    // convert::<f32>() should return magnitude
    let mag_block = r.convert::<f32>().read_volume().unwrap();
    for (i, val) in mag_block.data.iter().enumerate() {
        let expected = ((i as f32).powi(2) + ((i * 10) as f32).powi(2)).sqrt();
        assert!((val - expected).abs() < 1e-4);
    }
}

/// Write Float32Complex (Mode 4), read back.
#[test]
fn roundtrip_complex_f32() {
    let f = TempMrc::new("cpx_f32");
    let nx = 4;
    let ny = 4;
    let nz = 2;
    let total = nx * ny * nz;

    let src: Vec<Float32Complex> = (0..total)
        .map(|i| Float32Complex {
            real: i as f32,
            imag: (i as f32) * 1.5,
        })
        .collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<Float32Complex>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<Float32Complex>().unwrap();
    assert_eq!(block.data.len(), total);
    for (a, b) in block.data.iter().zip(src.iter()) {
        assert_eq!(a.real, b.real);
        assert_eq!(a.imag, b.imag);
    }
}

// ── MmapReader tests ──────────────────────────────────────────────────

#[cfg(feature = "mmap")]
#[test]
fn mmap_roundtrip_f32() {
    let f = TempMrc::new("mmap_f32");
    let nx = 16;
    let ny = 8;
    let nz = 4;
    let total = nx * ny * nz;

    let data: Vec<f32> = (0..total).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = MmapReader::open(f.path()).unwrap();
    let block = r.read_volume::<f32>().unwrap();
    assert_eq!(block.data, data);

    // Zero-copy slab_as
    let slab: &[f32] = r.slab_as::<f32>(0, 1).unwrap();
    assert_eq!(slab.len(), nx * ny);
    assert_eq!(slab, &data[..nx * ny]);
}

// ── Bzip2 roundtrip test ──────────────────────────────────────────────

#[cfg(feature = "bzip2")]
#[test]
fn roundtrip_bzip2() {
    let f = TempMrc::new("bzip2");
    let nx = 8;
    let ny = 8;
    let nz = 4;

    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish_bzip2()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    // Reader::open auto-detects bzip2
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<f32>().unwrap();
    assert_eq!(block.data, data);
}

// ── write_block_as roundtrip tests ────────────────────────────────────

#[test]
fn write_block_as_i16() {
    let f = TempMrc::new("wba_i16");
    let nx = 4;
    let ny = 4;
    let nz = 2;
    let total = nx * ny * nz;

    let src: Vec<f32> = (0..total).map(|i| (i as f32) - 8.0).collect();
    let expected_i16: Vec<i16> = src.iter().map(|&v| v as i16).collect();
    {
        let mut w = WriterBuilder::new(f.path())
            .shape([nx, ny, nz])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_block_as(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], src).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<i16>().unwrap();
    assert_eq!(block.data, expected_i16);
}

#[test]
fn write_block_as_u16() {
    let f = TempMrc::new("wba_u16");
    let total = 32usize;
    let src: Vec<f32> = (0..total).map(|i| (i * 100) as f32).collect();
    let expected_u16: Vec<u16> = src.iter().map(|&v| v as u16).collect();
    {
        let mut w = WriterBuilder::new(f.path())
            .shape([4, 4, 2])
            .mode::<u16>()
            .finish()
            .unwrap();
        w.write_block_as(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<u16>().unwrap();
    assert_eq!(block.data, expected_u16);
}

#[test]
fn write_block_as_i8() {
    let f = TempMrc::new("wba_i8");
    let total = 32usize;
    let src: Vec<f32> = (0..total).map(|i| (i as f32) - 16.0).collect();
    let expected_i8: Vec<i8> = src.iter().map(|&v| v as i8).collect();
    {
        let mut w = WriterBuilder::new(f.path())
            .shape([4, 4, 2])
            .mode::<i8>()
            .finish()
            .unwrap();
        w.write_block_as(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume::<i8>().unwrap();
    assert_eq!(block.data, expected_i8);
}

// ── Volume stack test ─────────────────────────────────────────────────

/// Verify that `volumes()` on a non-stack file returns the expected error.
#[test]
fn volume_stack_error_on_plain_volume() {
    let f = TempMrc::new("volstack_err");
    {
        let mut w = create(f.path())
            .shape([8, 8, 4])
            .mode::<f32>()
            .finish()
            .unwrap();
        let data = vec![0.0f32; 8 * 8 * 4];
        w.write_block(&VoxelBlock::new([0, 0, 0], [8, 8, 4], data).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    match r.volumes::<f32>() {
        Err(Error::NotAVolumeStack { .. }) => {} // expected
        other => panic!("expected NotAVolumeStack, got {:?}", other.map(|_| ())),
    }
}

// ── Permissive-mode edge case tests ───────────────────────────────────

#[test]
fn open_permissive_trailing_garbage() {
    let f = TempMrc::new("perm_garbage");
    {
        let mut w = create(f.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        let data = vec![1.0f32; 16];
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    // Append trailing garbage
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(f.path())
            .unwrap();
        file.write_all(b"TRAILING GARBAGE").unwrap();
    }
    // Strict mode should reject (file size mismatch)
    assert!(Reader::open(f.path()).is_err());
    // Permissive mode should still read correctly (ignores trailing data)
    let (reader, _warnings) = Reader::open_permissive(f.path()).unwrap();
    let block = reader.read_volume::<f32>().unwrap();
    assert_eq!(block.data, vec![1.0f32; 16]);
}

#[test]
fn open_permissive_bad_map() {
    let f = TempMrc::new("perm_map");
    // Write a valid file, then patch the MAP field in the header
    {
        let mut w = create(f.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        let data = vec![0.0f32; 16];
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    // Overwrite the MAP field (offset 208) with garbage
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(f.path())
            .unwrap();
        file.seek(SeekFrom::Start(208)).unwrap();
        file.write_all(b"XYZ_").unwrap();
    }

    // Strict mode should reject
    assert!(Reader::open(f.path()).is_err());
    // Permissive mode should succeed with warning about MAP field
    let (reader, warnings) = Reader::open_permissive(f.path()).unwrap();
    assert_eq!(reader.shape().nx, 4);
    let has_map_warning = warnings.iter().any(|w| w.contains("MAP"));
    assert!(
        has_map_warning,
        "expected MAP warning in permissive mode, got: {:?}",
        warnings
    );
}
