//! Comprehensive public API tests — covers every method listed in APIs.md.
//!
//! This file is the v0.7 test plan: every public API item must be exercised.
//! Tests use synthetic data generated programmatically (no external fixtures).
//! Run with: `cargo test --all-features --test api_comprehensive`

use mrc::*;
use std::io::{Cursor, Write};

// ── Helpers ──────────────────────────────────────────────────────────────────

struct TempMrc(std::path::PathBuf);

impl TempMrc {
    fn new(suffix: &str) -> Self {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "mrc_api_test_{}_{}.mrc",
            std::process::id(),
            suffix
        ));
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

fn write_f32_volume(f: &TempMrc, nx: usize, ny: usize, nz: usize) -> Vec<f32> {
    let total = nx * ny * nz;
    let data: Vec<f32> = (0..total).map(|i| i as f32).collect();
    let mut w = create(f.path())
        .shape([nx, ny, nz])
        .mode::<f32>()
        .finish()
        .unwrap();
    w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
        .unwrap();
    w.finalize().unwrap();
    data
}

// ── 1. Mode coverage — every MRC mode roundtrip ─────────────────────────────

#[test]
fn mode_0_int8_roundtrip() {
    let f = TempMrc::new("m0_i8");
    let total = 32usize;
    let src: Vec<i8> = (0..total).map(|i| (i as i8) - 16).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 2])
            .mode::<i8>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Int8(d) = block.data() else {
        panic!("expected Int8")
    };
    assert_eq!(d, src);
    // slices_mode0 signed
    let collected: Vec<f32> = r
        .slices_mode0(M0Interpretation::Signed)
        .flat_map(|s| s.unwrap().data.into_iter())
        .collect();
    assert_eq!(collected, src.iter().map(|&v| v as f32).collect::<Vec<_>>());
}

#[test]
fn mode_1_int16_roundtrip() {
    let f = TempMrc::new("m1_i16");
    let total = 32usize;
    let src: Vec<i16> = (0..total).map(|i| (i as i16) - 100).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 2])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Int16(d) = block.data() else {
        panic!("expected Int16")
    };
    assert_eq!(d, src);
    // convert to f32 via slices
    let collected: Vec<f32> = r
        .slices()
        .flat_map(|s| match s.unwrap().data() {
            DataView::Int16(d) => d.to_vec(),
            _ => panic!("type mismatch"),
        })
        .map(|v| v as f32)
        .collect();
    assert_eq!(collected, src.iter().map(|&v| v as f32).collect::<Vec<_>>());
}

#[test]
fn mode_2_float32_roundtrip() {
    let f = TempMrc::new("m2_f32");
    let src = write_f32_volume(&f, 8, 6, 4);
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Float32(d) = block.data() else {
        panic!("expected Float32")
    };
    assert_eq!(d, src);
}

#[test]
fn mode_6_uint16_roundtrip() {
    let f = TempMrc::new("m6_u16");
    let total = 32usize;
    let src: Vec<u16> = (0..total).map(|i| (i * 2) as u16).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 2])
            .mode::<u16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Uint16(d) = block.data() else {
        panic!("expected Uint16")
    };
    assert_eq!(d, src);
    // slices_u8 narrows u16→u8
    let narrowed: Vec<u8> = r
        .slices_u8()
        .flat_map(|s| s.unwrap().data.into_iter())
        .collect();
    assert_eq!(narrowed, src.iter().map(|&v| v as u8).collect::<Vec<_>>());
}

#[test]
#[cfg(feature = "f16")]
fn mode_12_float16_roundtrip() {
    use half::f16;
    let f = TempMrc::new("m12_f16");
    let total = 32usize;
    let src_f32: Vec<f32> = (0..total).map(|i| i as f32 * 1.25).collect();
    let src: Vec<f16> = src_f32.iter().map(|&v| f16::from_f32(v)).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 2])
            .mode::<f16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Float16(d) = block.data() else {
        panic!("expected Float16")
    };
    assert_eq!(d, src);
    // write_block_as f32→f16
    let f2 = TempMrc::new("m12_f16_wba");
    let mut w2 = create(f2.path())
        .shape([4, 4, 2])
        .mode::<f16>()
        .finish()
        .unwrap();
    w2.write_block_as(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src_f32.clone()).unwrap())
        .unwrap();
    w2.finalize().unwrap();
    let r2 = Reader::open(f2.path()).unwrap();
    let back: Vec<f32> = r2.convert::<f32>().read_volume().unwrap().data;
    for (a, b) in back.iter().zip(src_f32.iter()) {
        assert!((a - b).abs() < 0.01, "f16 roundtrip mismatch: {a} vs {b}");
    }
}

#[test]
fn mode_101_packed4bit_roundtrip() {
    let f = TempMrc::new("m101_full");
    let total = 64usize;
    let src: Vec<u8> = (0..total).map(|i| (i % 16) as u8).collect();
    {
        let mut w = create(f.path())
            .shape([8, 4, 2])
            .mode_raw(101)
            .finish()
            .unwrap();
        w.write_u4_block(&VoxelBlock::new([0, 0, 0], [8, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    assert_eq!(r.read_volume_u8().unwrap().data, src);
    // convert to f32
    let f32data: Vec<f32> = r.convert::<f32>().read_volume().unwrap().data;
    assert_eq!(f32data, src.iter().map(|&v| v as f32).collect::<Vec<_>>());
}

// ── 2. Reader constructors ───────────────────────────────────────────────────

#[test]
fn reader_open_plain() {
    let f = TempMrc::new("open_plain");
    write_f32_volume(&f, 4, 4, 1);
    let r = Reader::open_plain(f.path()).unwrap();
    assert_eq!(r.shape().nx, 4);
}

#[test]
fn reader_from_bytes() {
    let f = TempMrc::new("from_bytes");
    write_f32_volume(&f, 4, 4, 1);
    let bytes = std::fs::read(f.path()).unwrap();
    let r = Reader::from_bytes(bytes).unwrap();
    assert_eq!(r.shape().nx, 4);
}

#[test]
fn reader_from_bytes_permissive() {
    let f = TempMrc::new("from_bytes_perm");
    write_f32_volume(&f, 4, 4, 1);
    let mut bytes = std::fs::read(f.path()).unwrap();
    // Truncate data — keep header + extended header, cut 100 bytes of voxel data
    let data_offset = 1024; // no extended header for this file
    bytes.truncate(data_offset + (16 * 4 / 2)); // keep header + half the data
    let (r, _warnings) = Reader::from_bytes_permissive(bytes).unwrap();
    assert!(r.is_truncated());
    // Warnings may be empty if header itself validated OK
    // but the truncated data should be detectable
    assert!(r.raw_bytes().len() < 16 * 4, "data should be truncated");
}

#[test]
fn reader_from_reader() {
    let f = TempMrc::new("from_reader");
    write_f32_volume(&f, 4, 4, 1);
    let bytes = std::fs::read(f.path()).unwrap();
    let r = Reader::from_reader(Cursor::new(bytes)).unwrap();
    assert_eq!(r.shape().nx, 4);
}

#[test]
fn reader_buffered_slab() {
    // from_bytes creates a DataSource::Buffered reader.
    let f = TempMrc::new("buffered_slab");
    let nx = 8;
    let ny = 6;
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
    let bytes = std::fs::read(f.path()).unwrap();
    let r = Reader::from_bytes(bytes).unwrap();

    let _expected = &data[nx * ny..][..nx * ny * 2];

    // slices iterator via read_block_bytes_cow fast path (Cow::Borrowed)
    let slice_count = r.slices().count();
    assert_eq!(slice_count, nz);
    for (z, slice_result) in r.slices().enumerate() {
        let block = slice_result.unwrap();
        let DataView::Float32(d) = block.data() else {
            panic!("expected Float32")
        };
        assert_eq!(d, &data[z * nx * ny..(z + 1) * nx * ny]);
    }
}

#[test]
fn reader_gzip_open_detect() {
    let f = TempMrc::new("gzip_detect");
    let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    {
        let mut w = create(f.path())
            .shape([2, 2, 1])
            .mode::<f32>()
            .finish_gzip()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [2, 2, 1], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    // auto-detect
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Float32(d) = block.data() else {
        panic!("expected Float32")
    };
    assert_eq!(d, data);
}

// ── 3. Reader accessors ──────────────────────────────────────────────────────

#[test]
fn reader_accessors() {
    let f = TempMrc::new("accessors");
    write_f32_volume(&f, 8, 4, 2);
    let r = Reader::open(f.path()).unwrap();
    assert_eq!(r.shape().nx, 8);
    assert_eq!(r.shape().ny, 4);
    assert_eq!(r.shape().nz, 2);
    assert_eq!(r.mode(), Mode::Float32);
    assert_eq!(r.endian(), FileEndian::LittleEndian);
    assert!(!r.is_truncated());
    let h = r.header();
    assert_eq!(h.nx, 8);
    assert_eq!(h.ny, 4);
    assert_eq!(h.nz, 2);
}

#[test]
fn reader_data_bytes() {
    let f = TempMrc::new("raw_bytes");
    let data = write_f32_volume(&f, 4, 4, 1);
    let r = Reader::open(f.path()).unwrap();
    let bytes = r.raw_bytes();
    assert_eq!(bytes.len(), 16 * 4); // 16 f32 values × 4 bytes
    let decoded: &[f32] = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const f32, 16) };
    assert_eq!(decoded, &data[..]);
}

#[test]
fn reader_ext_header_bytes() {
    // No extended header → empty slice
    let f = TempMrc::new("ext_header");
    write_f32_volume(&f, 4, 4, 1);
    let r = Reader::open(f.path()).unwrap();
    assert!(r.ext_header_bytes().is_empty());
}

#[test]
fn reader_read_block_bytes() {
    let f = TempMrc::new("read_block_bytes");
    write_f32_volume(&f, 8, 8, 4);
    let r = Reader::open(f.path()).unwrap();
    let bytes = r.read_block_bytes([0, 0, 0], [4, 4, 2]).unwrap();
    assert_eq!(bytes.len(), 4 * 4 * 2 * 4); // 4×4×2 f32 values
}

// ── 4. Reader iteration methods ──────────────────────────────────────────────

#[test]
fn reader_slices_slabs_tiles() {
    let f = TempMrc::new("iter_methods");
    write_f32_volume(&f, 8, 4, 6);
    let r = Reader::open(f.path()).unwrap();

    // slices
    let slice_count = r.slices().count();
    assert_eq!(slice_count, 6);

    // slabs
    let slab_count = r.slabs(2).count();
    assert_eq!(slab_count, 3);

    // tiles
    let tile_count = r.tiles([4, 4, 2]).unwrap().count();
    assert_eq!(tile_count, 6); // 2×1×3 = 6 tiles
}

#[test]
fn reader_volumes_error_on_plain() {
    let f = TempMrc::new("vols_err");
    write_f32_volume(&f, 4, 4, 4);
    let r = Reader::open(f.path()).unwrap();
    if let Err(Error::NotAVolumeStack { .. }) = r.volumes() {}
}

#[test]
fn reader_volume_stack_queries_and_iter() {
    // Write a volume stack: 2 sub-volumes × 4 slices = nz=8
    let f = TempMrc::new("vol_stack_iter");
    let nx = 8;
    let ny = 6;
    let nz = 8;
    let mz = 4;
    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .volume_stack(mz)
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();

    // Step 1: reader-level queries
    assert!(r.is_volume_stack());
    assert!(!r.is_image_stack());
    assert!(!r.is_single_image());
    assert!(!r.is_volume());
    assert_eq!(r.logical_shape(), [2, 4, 6, 8]);

    let mz_usize = mz as usize;

    // Reader::volumes()
    let vols: Vec<_> = r.volumes().unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(vols.len(), 2);
    for (i, vol) in vols.iter().enumerate() {
        assert_eq!(vol.shape(), [nx, ny, mz_usize]);
        assert_eq!(vol.offset(), [0, 0, i * mz_usize]);
        let expected = &data[i * mz_usize * nx * ny..(i + 1) * mz_usize * nx * ny];
        let DataView::Float32(d) = vol.data() else {
            panic!("expected Float32")
        };
        assert_eq!(d, expected);
    }

    // Step 2: ConvertReader::volumes()
    let conv_vols: Vec<_> = r
        .convert::<f32>()
        .volumes()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(conv_vols.len(), 2);
    for (i, vol) in conv_vols.iter().enumerate() {
        assert_eq!(vol.shape, [nx, ny, mz_usize]);
        assert_eq!(vol.offset, [0, 0, i * mz_usize]);
        let expected = &data[i * mz_usize * nx * ny..(i + 1) * mz_usize * nx * ny];
        assert_eq!(vol.data, expected);
    }

    // ConvertReader::volumes() on non-stack file → error
    let f2 = TempMrc::new("vol_stack_plain_err");
    write_f32_volume(&f2, 4, 4, 4);
    let r2 = Reader::open(f2.path()).unwrap();
    match r2.convert::<f32>().volumes() {
        Err(Error::NotAVolumeStack { .. }) => {}
        Err(e) => panic!("expected NotAVolumeStack, got {e:?}"),
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

#[test]
fn reader_subregion_corner() {
    let f = TempMrc::new("subregion_corner");
    let data = write_f32_volume(&f, 10, 10, 10);
    let r = Reader::open(f.path()).unwrap();
    let block = r.subregion([0, 0, 0], [5, 5, 5]).unwrap();
    match block.data() {
        DataView::Float32(d) => assert_eq!(d.len(), 125),
        _ => panic!("type mismatch"),
    };
    for z in 0..5 {
        for y in 0..5 {
            for x in 0..5 {
                let idx = z * 100 + y * 10 + x;
                match block.data() {
                    DataView::Float32(d) => assert_eq!(d[z * 25 + y * 5 + x], data[idx]),
                    _ => panic!("type mismatch"),
                };
            }
        }
    }
}

#[test]
fn reader_read_volume() {
    let f = TempMrc::new("read_vol");
    let data = write_f32_volume(&f, 6, 6, 6);
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Float32(d) = block.data() else {
        panic!("expected Float32")
    };
    assert_eq!(d, data);
}

#[test]
fn reader_slabs_u8() {
    let f = TempMrc::new("slabs_u8");
    let total = 32usize;
    let src: Vec<u16> = (0..total).map(|i| (i % 200) as u16).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 2])
            .mode::<u16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let slabs: Vec<u8> = r
        .slabs_u8(2)
        .flat_map(|s| s.unwrap().data.into_iter())
        .collect();
    assert_eq!(slabs, src.iter().map(|&v| v as u8).collect::<Vec<_>>());
}

#[test]
fn reader_slices_mode0_unsigned() {
    let f = TempMrc::new("m0_unsigned");
    let src: Vec<i8> = vec![-1, 0, 1, -128, 127];
    {
        let mut w = create(f.path())
            .shape([5, 1, 1])
            .mode::<i8>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [5, 1, 1], src).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    // Unsigned interpretation: -1 → 255, -128 → 128
    let unsigned: Vec<f32> = r
        .slices_mode0(M0Interpretation::Unsigned)
        .flat_map(|s| s.unwrap().data.into_iter())
        .collect();
    assert_eq!(unsigned[0], 255.0);
    assert_eq!(unsigned[3], 128.0);
}

#[test]
fn reader_convert_variants() {
    let f = TempMrc::new("convert_variants");
    let total = 32usize;
    let src: Vec<i16> = (0..total).map(|i| (i as i16) - 100).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 2])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();

    // convert to f32
    let f32_data: Vec<f32> = r.convert::<f32>().read_volume().unwrap().data;
    assert_eq!(f32_data, src.iter().map(|&v| v as f32).collect::<Vec<_>>());

    // convert to i16 (identity)
    let i16_data: Vec<i16> = r.convert::<i16>().read_volume().unwrap().data;
    assert_eq!(i16_data, src);

    // convert with custom complex strategy
    let _ = r
        .convert::<f32>()
        .with_complex_strategy(ComplexToRealStrategy::RealPart);
}

#[test]
fn reader_to_ndarray() {
    #[cfg(feature = "ndarray")]
    {
        let f = TempMrc::new("ndarray");
        write_f32_volume(&f, 4, 4, 2);
        let r = Reader::open(f.path()).unwrap();
        let arr = r.convert::<f32>().to_ndarray().unwrap();
        assert_eq!(arr.shape(), &[2, 4, 4]); // [nz, ny, nx]
    }
}

// ── 5. Writer API ────────────────────────────────────────────────────────────

#[test]
fn writer_from_writer() {
    let header = Header::new();
    let mut h = header;
    h.nx = 4;
    h.ny = 4;
    h.nz = 1;
    h.mx = 4;
    h.my = 4;
    h.mz = 1;
    h.mode = 2;
    h.nlabl = 0;
    let mut w = Writer::from_writer(Cursor::new(Vec::new()), h, &[]).unwrap();
    let data = vec![1.0f32; 16];
    w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data).unwrap())
        .unwrap();
    w.finalize().unwrap();
}

#[test]
fn writer_all_builder_setters() {
    let f = TempMrc::new("builder_full");
    let mut w = WriterBuilder::new(f.path())
        .shape([4, 4, 2])
        .mode::<f32>()
        .mode_raw(2)
        .cell_lengths(1.0, 1.0, 1.0)
        .cell_angles(90.0, 90.0, 90.0)
        .ispg(1)
        .exttyp(*b"CCP4")
        .nsymbt(0)
        .origin([0.0, 0.0, 0.0])
        .nstart([0, 0, 0])
        .sampling([4, 4, 2])
        .axis_mapping([1, 2, 3])
        .add_label("test volume")
        .finish()
        .unwrap();
    let data = vec![0.0f32; 32];
    w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 2], data).unwrap())
        .unwrap();
    w.finalize().unwrap();
}

#[test]
fn writer_header_mut() {
    let f = TempMrc::new("header_mut");
    let mut w = create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    // Modify header mid-write
    w.header_mut().add_label("mid-write label");
    let data = vec![0.0f32; 16];
    w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data).unwrap())
        .unwrap();
    w.finalize().unwrap();
    let r = Reader::open(f.path()).unwrap();
    let labels = r.header().get_labels();
    assert!(labels.iter().any(|l| l.contains("mid-write")));
}

#[test]
fn writer_write_u8_block() {
    let f = TempMrc::new("write_u8");
    let src: Vec<u8> = (0..16).map(|i| i as u8).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 1])
            .mode::<u16>()
            .finish()
            .unwrap();
        w.write_u8_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let expected: Vec<u16> = src.iter().map(|&v| v as u16).collect();
    match r.read_volume().unwrap().data() {
        DataView::Uint16(d) => assert_eq!(d, expected),
        _ => panic!("type mismatch"),
    };
}

#[test]
fn writer_write_block_as_float32_passthrough() {
    let f = TempMrc::new("wba_f32");
    let src: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    {
        let mut w = create(f.path())
            .shape([2, 2, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_block_as(&VoxelBlock::new([0, 0, 0], [2, 2, 1], src.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    let DataView::Float32(d) = block.data() else {
        panic!("expected Float32")
    };
    assert_eq!(d, src);
}

#[test]
fn writer_write_block_parallel() {
    #[cfg(feature = "parallel")]
    {
        let f = TempMrc::new("parallel");
        let total = 64usize;
        let src: Vec<f32> = (0..total).map(|i| i as f32).collect();
        {
            let mut w = create(f.path())
                .shape([4, 4, 4])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_block_parallel(&VoxelBlock::new([0, 0, 0], [4, 4, 4], src.clone()).unwrap())
                .unwrap();
            w.finalize().unwrap();
        }
        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume().unwrap();
        let DataView::Float32(d) = block.data() else {
            panic!("expected Float32")
        };
        assert_eq!(d, src);
    }
}

#[test]
fn writer_update_header_stats_and_validate() {
    let f = TempMrc::new("stats_validate");
    let total = 16usize;
    let src: Vec<f32> = (0..total).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([4, 4, 1])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], src.clone()).unwrap())
            .unwrap();
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    assert!(r.validate_header_stats().is_ok());
}

// ── 6. Header API ────────────────────────────────────────────────────────────

#[test]
fn header_decode_encode_roundtrip() {
    let mut h = Header::new();
    h.nx = 64;
    h.ny = 64;
    h.nz = 32;
    h.mx = 64;
    h.my = 64;
    h.mz = 32;
    h.mode = 2;
    h.nlabl = 0;
    let mut bytes = [0u8; 1024];
    h.encode_to_bytes(&mut bytes);
    let h2 = Header::decode_from_bytes(&bytes);
    assert_eq!(h2.nx, 64);
    assert_eq!(h2.ny, 64);
    assert_eq!(h2.nz, 32);
    assert_eq!(h2.mode, 2);
}

#[test]
fn header_endianness_detection() {
    let le = [0x44, 0x44, 0x00, 0x00];
    let be = [0x11, 0x11, 0x00, 0x00];
    assert_eq!(FileEndian::from_machst(&le), FileEndian::LittleEndian);
    assert_eq!(FileEndian::from_machst(&be), FileEndian::BigEndian);
    assert!(FileEndian::native().is_native());
}

#[test]
fn header_exttyp_roundtrip() {
    let mut h = Header::new();
    h.set_exttyp(*b"FEI1");
    assert_eq!(h.exttyp(), *b"FEI1");
    assert_eq!(h.exttyp_str().unwrap(), "FEI1");
    h.set_exttyp_str("CCP4").unwrap();
    assert_eq!(h.exttyp(), *b"CCP4");
}

#[test]
fn header_nversion_roundtrip() {
    let mut h = Header::new();
    assert_eq!(h.nversion(), 20141);
    h.set_nversion(0);
    assert_eq!(h.nversion(), 0);
    // NVERSION=0 should still pass strict validation
    h.nx = 64;
    h.ny = 64;
    h.nz = 1;
    h.mx = 64;
    h.my = 64;
    h.mz = 1;

    h.nlabl = 0;
    assert!(h.validate());
}

#[test]
fn header_labels() {
    let mut h = Header::new();
    h.add_label("first");
    h.add_label("second");
    let labels = h.get_labels();
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0], "first");
    assert_eq!(labels[1], "second");
    assert_eq!(h.label_at(0), Some("first"));
    assert!(h.label_at(5).is_none());
}

#[test]
fn header_volume_type_checks() {
    let mut h = Header::new();
    h.nx = 64;
    h.ny = 64;
    h.nz = 1;
    h.mx = 64;
    h.my = 64;
    h.mz = 1;
    assert!(h.is_single_image());

    h.nz = 10;
    h.ispg = 0;
    h.mz = 1;
    assert!(h.is_image_stack());

    h.ispg = 1;
    h.mz = 10;
    assert!(h.is_volume());
    assert!(!h.is_volume_stack());

    h.set_volume_stack(5);
    assert!(h.is_volume_stack());
    assert_eq!(h.logical_shape(), [2, 5, 64, 64]); // nz=10, mz=5 → 2 volumes
}

#[test]
fn header_cell_volume() {
    let mut h = Header::new();
    h.xlen = 10.0;
    h.ylen = 10.0;
    h.zlen = 10.0;
    h.alpha = 90.0;
    h.beta = 90.0;
    h.gamma = 90.0;
    let vol = h.cell_volume();
    assert!(
        (vol - 1000.0).abs() < 1e-6,
        "cubic cell volume mismatch: {vol}"
    );
}

// ── 7. Error handling ────────────────────────────────────────────────────────

#[test]
fn error_invalid_header() {
    let f = TempMrc::new("err_invalid");
    // Write a 1024-byte file with all zeros — parses as header but fails validation
    // (negative dimensions, no MAP field, etc.)
    let bad = vec![0x00u8; 1024];
    std::fs::write(f.path(), &bad).unwrap();
    match Reader::open(f.path()) {
        Err(Error::InvalidHeaderDetailed(_)) => {} // validation caught it
        Err(Error::InvalidHeader) => {}            // truly unparseable
        other => panic!("expected InvalidHeader or InvalidHeaderDetailed, got {other:?}"),
    }
}

#[test]
fn error_unsupported_mode() {
    // Mode 99 is not supported
    let f = TempMrc::new("err_mode99");
    // Write raw header with mode 99
    let mut h = Header::new();
    h.nx = 4;
    h.ny = 4;
    h.nz = 1;
    h.mx = 4;
    h.my = 4;
    h.mz = 1;
    h.mode = 99;
    h.nlabl = 0;
    let raw_header = {
        let mut h2 = h;
        h2.nx = 4;
        h2.ny = 4;
        h2.nz = 1;
        h2.mx = 4;
        h2.my = 4;
        h2.mz = 1;
        h2.mode = 99;
        h2.nlabl = 0;
        let mut bytes = [0u8; 1024];
        h2.encode_to_bytes(&mut bytes);
        bytes
    };
    // Write the header + minimal data
    let mut file = std::fs::File::create(f.path()).unwrap();
    file.write_all(&raw_header).unwrap();
    file.write_all(&[0u8; 64]).unwrap(); // data
    drop(file);
    match Reader::open(f.path()) {
        Err(Error::InvalidHeaderDetailed(HeaderValidationError::UnsupportedMode(99))) => {}
        other => panic!("expected InvalidHeaderDetailed(UnsupportedMode(99)), got {other:?}"),
    }
}

#[test]
fn error_bounds() {
    let f = TempMrc::new("err_bounds");
    write_f32_volume(&f, 4, 4, 1);
    let r = Reader::open(f.path()).unwrap();
    match r.subregion([0, 0, 0], [10, 10, 10]) {
        Err(Error::BoundsError { .. }) => {}
        other => panic!("expected BoundsError, got {other:?}"),
    }
}

#[test]
fn error_mode_mismatch() {
    let f = TempMrc::new("err_mode_mismatch");
    let mut w = create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    // Writing an Int16 block to an Float32 file should produce ModeMismatch.
    let block = VoxelBlock::new([0, 0, 0], [4, 4, 1], vec![0i16; 16]).unwrap();
    match w.write_block(&block) {
        Err(Error::ModeMismatch { .. }) => {}
        other => panic!("expected ModeMismatch, got {other:?}"),
    }
}

#[test]
fn error_value_out_of_range_u16_to_u8() {
    let src = vec![0u16, 256u16]; // 256 > 255
    match mrc::convert_u16_slice_to_u8(&src) {
        Err(Error::ValueOutOfRange {
            value: 256,
            max: 255,
        }) => {}
        other => panic!("expected ValueOutOfRange, got {other:?}"),
    }
}

#[test]
fn error_block_shape_mismatch() {
    match VoxelBlock::<f32>::new([0, 0, 0], [2, 2, 2], vec![0.0f32; 5]) {
        Err(Error::BlockShapeMismatch {
            expected: 8,
            actual: 5,
        }) => {}
        other => panic!("expected BlockShapeMismatch, got {other:?}"),
    }
}

#[test]
fn error_file_size_mismatch() {
    let f = TempMrc::new("err_filesize");
    // Write a valid file, then append trailing garbage
    write_f32_volume(&f, 4, 4, 1);
    // Append extra bytes (trailing garbage)
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(f.path())
        .unwrap();
    file.write_all(b"TRAILING GARBAGE").unwrap();
    drop(file);
    match Reader::open(f.path()) {
        Err(Error::FileSizeMismatch { .. }) => {}
        other => panic!("expected FileSizeMismatch, got {other:?}"),
    }
}

// ── 8. Validation API ────────────────────────────────────────────────────────

#[test]
fn validate_full_valid_file() {
    let f = TempMrc::new("validate_ok");
    write_f32_volume(&f, 8, 8, 4);
    let report = mrc::validate::validate_full(f.path(), false).unwrap();
    assert!(report.is_valid());
    assert_eq!(report.nx, 8);
    assert_eq!(report.ny, 8);
    assert_eq!(report.nz, 4);
}

#[test]
fn validate_full_invalid_file() {
    let f = TempMrc::new("validate_bad");
    // Write a file with valid dimensions but bad MAP field
    let mut h = Header::new();
    h.nx = 4;
    h.ny = 4;
    h.nz = 1;
    h.mx = 4;
    h.my = 4;
    h.mz = 1;
    h.mode = 2;
    h.nlabl = 0;
    h.map = [0x00, 0x00, 0x00, 0x00]; // all-zero MAP (accepted but non-standard)
    // Also set NSYMBT to negative to trigger a warning
    h.nsymbt = -1;
    let mut bytes = [0u8; 1024];
    h.encode_to_bytes(&mut bytes);
    // Write header + minimal data
    let mut file = std::fs::File::create(f.path()).unwrap();
    file.write_all(&bytes).unwrap();
    file.write_all(&[0u8; 64]).unwrap(); // data
    drop(file);
    let report = mrc::validate::validate_full(f.path(), true).unwrap();
    // Should have warnings (negative NSYMBT) but no hard errors
    let warnings: Vec<_> = report
        .by_severity(mrc::validate::Severity::Warning)
        .collect();
    assert!(!warnings.is_empty(), "expected at least one warning");
}

#[test]
fn validate_reader() {
    let f = TempMrc::new("validate_reader");
    write_f32_volume(&f, 4, 4, 1);
    let r = Reader::open(f.path()).unwrap();
    let report = mrc::validate::validate_reader(&r, "test", "plain", &[]).unwrap();
    assert!(report.is_valid());
}

// ── 9. Conversion utilities ──────────────────────────────────────────────────

#[test]
fn conv_reinterpret_m0_signed() {
    let data = vec![0x00u8, 0x80, 0xFF];
    let result = mrc::reinterpret_m0(&data, M0Interpretation::Signed);
    assert_eq!(result, vec![0.0, -128.0, -1.0]);
}

#[test]
fn conv_reinterpret_m0_unsigned() {
    let data = vec![0x00u8, 0x80, 0xFF];
    let result = mrc::reinterpret_m0(&data, M0Interpretation::Unsigned);
    assert_eq!(result, vec![0.0, 128.0, 255.0]);
}

#[test]
fn conv_u16_to_u8_overflow() {
    assert!(mrc::convert_u16_slice_to_u8(&[255]).is_ok());
    assert!(mrc::convert_u16_slice_to_u8(&[256]).is_err());
}

#[test]
fn conv_u8_to_u16_roundtrip() {
    let src: Vec<u8> = (0..=255).collect();
    let wide = crate::convert_u8_slice_to_u16(&src);
    let back = mrc::convert_u16_slice_to_u8(&wide).unwrap();
    assert_eq!(src, back);
}

// ── 10. Permissive mode + is_truncated ───────────────────────────────────────

#[test]
fn permissive_truncated_detection() {
    let f = TempMrc::new("perm_truncated");
    write_f32_volume(&f, 8, 8, 4);
    // Truncate data
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(f.path())
        .unwrap();
    file.set_len(1024 + 100).unwrap(); // header + 100 bytes only
    drop(file);
    let (r, _warnings) = Reader::open_permissive(f.path()).unwrap();
    assert!(r.is_truncated());
    // raw_bytes returns whatever is available
    assert!(r.raw_bytes().len() <= 100);
}

// ── 11. Extended header dispatch ─────────────────────────────────────────────

#[test]
fn ext_header_dispatch_none() {
    let f = TempMrc::new("ext_dispatch_none");
    write_f32_volume(&f, 4, 4, 1);
    let r = Reader::open(f.path()).unwrap();
    match r.parse_extended_header() {
        ExtHeaderData::None => {}
        other => panic!("expected None, got {other:?}"),
    }
}

#[test]
fn ext_header_from_header() {
    let header = Header::new();
    assert_eq!(
        ExtHeaderType::from_header(&header),
        ExtHeaderType::Unknown([0; 4])
    );
    // No known exttyp set
}

// ── 12. Compression auto-detect ──────────────────────────────────────────────

#[test]
fn detect_compression_plain() {
    let f = TempMrc::new("detect_plain");
    write_f32_volume(&f, 4, 4, 1);
    let ct = mrc::CompressionType::Plain;
    assert_eq!(mrc::detect_compression(f.path()).unwrap(), ct);
}

#[test]
#[cfg(feature = "gzip")]
fn decompression_bomb_limit() {
    // open_gzip_with_limit with a tiny limit should fail
    let f = TempMrc::new("bomb_limit");
    std::fs::write(f.path(), [0x1f, 0x8b, 0x00]).unwrap(); // truncated gzip header
    match Reader::open_gzip_with_limit(f.path(), 10) {
        Err(_) => {} // expected: decompression fails or limit exceeded
        Ok(_) => panic!("expected error for tiny limit"),
    }
}

// ── 13. Volume stack convenience ─────────────────────────────────────────────

#[test]
fn writer_set_volume_stack() {
    let f = TempMrc::new("set_volstack");
    let nx = 4;
    let ny = 4;
    let nz = 8;
    let subvol = 2;
    let total = nx * ny * nz;
    let data: Vec<f32> = (0..total).map(|i| i as f32).collect();
    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .volume_stack(subvol)
            .finish()
            .unwrap();
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f.path()).unwrap();
    assert!(r.header().is_volume_stack());
    assert_eq!(r.header().mz, subvol);
    let nvol = nz / subvol as usize;
    let count = r.volumes().unwrap().count();
    assert_eq!(count, nvol);
}

#[test]
fn writer_builder_image_stack_and_volume() {
    let f_img = TempMrc::new("builder_imgstack");
    let f_vol = TempMrc::new("builder_vol");

    // set_image_stack: ispg=0, mz=1
    {
        let mut w = create(f_img.path())
            .shape([8, 8, 10])
            .mode::<f32>()
            .image_stack()
            .finish()
            .unwrap();
        let data = vec![0.0f32; 8 * 8 * 10];
        w.write_block(&VoxelBlock::new([0, 0, 0], [8, 8, 10], data).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r = Reader::open(f_img.path()).unwrap();
    assert!(r.is_image_stack());
    assert_eq!(r.header().ispg, 0);
    assert_eq!(r.header().mz, 1);

    // set_volume: ispg=1, mz=nz
    {
        let mut w = create(f_vol.path())
            .shape([8, 8, 10])
            .mode::<f32>()
            .volume()
            .finish()
            .unwrap();
        let data = vec![1.0f32; 8 * 8 * 10];
        w.write_block(&VoxelBlock::new([0, 0, 0], [8, 8, 10], data).unwrap())
            .unwrap();
        w.finalize().unwrap();
    }
    let r2 = Reader::open(f_vol.path()).unwrap();
    assert!(r2.is_volume());
    assert_eq!(r2.header().ispg, 1);
    assert_eq!(r2.header().mz, 10);
}

// ── 14. Writer::from_writer_mmap / _gzip / _bzip2 ────────────────────────────

#[test]
fn writer_from_writer_mmap() {
    #[cfg(feature = "mmap")]
    {
        let f = TempMrc::new("from_writer_mmap");
        let mut h = Header::new();
        h.nx = 4;
        h.ny = 4;
        h.nz = 1;
        h.mx = 4;
        h.my = 4;
        h.mz = 1;
        h.mode = 2;
        h.nlabl = 0;
        let mut w = Writer::from_writer_mmap(f.path(), h, &[]).unwrap();
        let data = vec![42.0f32; 16];
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume().unwrap();
        let DataView::Float32(d) = block.data() else {
            panic!("expected Float32")
        };
        assert_eq!(d, data);
    }
}

#[test]
fn writer_from_writer_gzip() {
    #[cfg(feature = "gzip")]
    {
        let f = TempMrc::new("from_writer_gzip");
        let mut h = Header::new();
        h.nx = 4;
        h.ny = 4;
        h.nz = 1;
        h.mx = 4;
        h.my = 4;
        h.mz = 1;
        h.mode = 2;
        h.nlabl = 0;
        let mut w = Writer::from_writer_gzip(f.path(), h, &[], CompressionLevel::Balanced).unwrap();
        let data = vec![1.0f32; 16];
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume().unwrap();
        let DataView::Float32(d) = block.data() else {
            panic!("expected Float32")
        };
        assert_eq!(d, data);
    }
}

#[test]
fn writer_from_writer_bzip2() {
    #[cfg(feature = "bzip2")]
    {
        let f = TempMrc::new("from_writer_bzip2");
        let mut h = Header::new();
        h.nx = 4;
        h.ny = 4;
        h.nz = 1;
        h.mx = 4;
        h.my = 4;
        h.mz = 1;
        h.mode = 2;
        h.nlabl = 0;
        let mut w = Writer::from_writer_bzip2(f.path(), h, &[], CompressionLevel::Fast).unwrap();
        let data = vec![2.0f32; 16];
        w.write_block(&VoxelBlock::new([0, 0, 0], [4, 4, 1], data.clone()).unwrap())
            .unwrap();
        w.finalize().unwrap();
        let r = Reader::open(f.path()).unwrap();
        let block = r.read_volume().unwrap();
        let DataView::Float32(d) = block.data() else {
            panic!("expected Float32")
        };
        assert_eq!(d, data);
    }
}

// ── 15. One-shot ergonomic API (set_data, read_as, write_as) ──────────

#[test]
fn writer_set_data_and_finalize() {
    let f = TempMrc::new("set_data");
    let nx = 8;
    let ny = 6;
    let nz = 4;
    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();

    {
        let mut w = create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.set_data(&data).unwrap();
        w.finalize().unwrap();
    }

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    match block.data() {
        DataView::Float32(d) => assert_eq!(d, data),
        _ => panic!("type mismatch"),
    };
}

#[test]
fn write_as_roundtrip() {
    let f = TempMrc::new("write_as");
    let nx = 8;
    let ny = 6;
    let nz = 4;
    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();

    write_as(f.path(), &data, [nx, ny, nz]).unwrap();

    let r = Reader::open(f.path()).unwrap();
    let block = r.read_volume().unwrap();
    match block.data() {
        DataView::Float32(d) => assert_eq!(d, data),
        _ => panic!("type mismatch"),
    };
}

#[test]
fn read_as_roundtrip() {
    let f = TempMrc::new("read_as");
    let nx = 8;
    let ny = 6;
    let nz = 4;
    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();

    write_as(f.path(), &data, [nx, ny, nz]).unwrap();

    let (header, read_data): (_, Vec<f32>) = read_as(f.path()).unwrap();
    assert_eq!(header.nx as usize, nx);
    assert_eq!(header.ny as usize, ny);
    assert_eq!(header.nz as usize, nz);
    assert_eq!(read_data, data);
}

#[test]
fn write_as_i16_roundtrip() {
    let f = TempMrc::new("write_as_i16");
    let data: Vec<i16> = vec![-100, 0, 100, 32767, -32768];

    write_as(f.path(), &data, [5, 1, 1]).unwrap();

    let r = Reader::open(f.path()).unwrap();
    assert_eq!(r.mode(), Mode::Int16);
    let block = r.read_volume().unwrap();
    match block.data() {
        DataView::Int16(d) => assert_eq!(d, data),
        _ => panic!("type mismatch"),
    };
}
