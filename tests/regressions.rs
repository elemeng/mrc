//! Regression tests for correctness and API-contract fixes.

use mrc::{
    ComplexToRealStrategy, DataBlock, DataView, FileEndian, Header, Int16Complex, Mode, OwnedData,
    Reader, VoxelBlock,
};
use std::path::{Path, PathBuf};

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "mrc_regression_{}_{}.mrc",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_file(&p);
        Self(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn header_bytes(header: Header) -> [u8; 1024] {
    let mut bytes = [0u8; 1024];
    header.encode_to_bytes(&mut bytes);
    bytes
}

fn base_header() -> Header {
    let mut h = Header::new();
    h.nx = 4;
    h.ny = 4;
    h.nz = 1;
    h.mx = 4;
    h.my = 4;
    h.mz = 1;
    h.mode = 2;
    h
}

/// A valid file whose extended header is 1 byte long places the voxel payload
/// at byte 1025. Typed conversion must not create a misaligned `&[i16]`.
#[test]
#[cfg(feature = "mmap")]
fn misaligned_ext_header_conversion_is_safe() {
    let f = TempFile::new("misaligned");
    let mut h = base_header();
    h.nx = 2;
    h.ny = 2;
    h.mx = 2;
    h.my = 2;
    h.mode = Mode::Int16.as_i32();
    h.nsymbt = 1;

    let mut bytes = header_bytes(h).to_vec();
    bytes.push(0); // one-byte extended header
    for v in [1i16, 2, 3, 4] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(f.path(), &bytes).unwrap();

    let reader = Reader::open(f.path()).unwrap();
    assert_eq!(reader.raw_bytes().as_ptr() as usize % 2, 1);
    let block = reader.convert::<f32>().read_volume().unwrap();
    assert_eq!(block.data, vec![1.0, 2.0, 3.0, 4.0]);
}

/// Big-endian Mode 3 data must keep real and imaginary components in place.
#[test]
fn big_endian_complex_components_are_preserved() {
    let f = TempFile::new("be_complex");
    let mut h = base_header();
    h.nx = 2;
    h.ny = 1;
    h.mx = 2;
    h.my = 1;
    h.mode = Mode::Int16Complex.as_i32();
    h.set_file_endian(FileEndian::BigEndian);

    let mut bytes = header_bytes(h).to_vec();
    for (real, imag) in [(1i16, 2i16), (-3, 4)] {
        bytes.extend_from_slice(&real.to_be_bytes());
        bytes.extend_from_slice(&imag.to_be_bytes());
    }
    std::fs::write(f.path(), &bytes).unwrap();

    let reader = Reader::open(f.path()).unwrap();
    assert_eq!(reader.endian(), FileEndian::BigEndian);

    let block = reader.subregion([0, 0, 0], [2, 1, 1]).unwrap();
    let DataView::Int16Complex(data) = block.data() else {
        panic!("expected Int16Complex");
    };
    assert_eq!(
        data,
        &[
            Int16Complex { real: 1, imag: 2 },
            Int16Complex { real: -3, imag: 4 }
        ]
    );

    let converted = reader
        .convert::<f32>()
        .with_complex_strategy(ComplexToRealStrategy::RealPart)
        .read_volume()
        .unwrap();
    assert_eq!(converted.data, vec![1.0, -3.0]);
}

#[test]
fn swap_bytes_in_place_handles_complex_components() {
    let mut data = [Int16Complex {
        real: 0x1234,
        imag: 0x5678,
    }];
    mrc::swap_bytes_in_place(&mut data, FileEndian::BigEndian);
    assert_eq!(data[0].real, 0x3412);
    assert_eq!(data[0].imag, 0x7856);
}

#[test]
fn write_data_block_rejects_mode_mismatch() {
    let f = TempFile::new("wrong_mode");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    let block = DataBlock::Owned {
        offset: [0, 0, 0],
        shape: [4, 4, 1],
        data: OwnedData::Int16(vec![7; 16]),
    };
    let err = w.write_data_block(&block).unwrap_err();
    assert!(matches!(err, mrc::Error::ModeMismatch { .. }));
}

#[test]
fn write_data_block_rejects_wrong_length() {
    let f = TempFile::new("wrong_len");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    let block = DataBlock::Owned {
        offset: [0, 0, 0],
        shape: [4, 4, 1],
        data: OwnedData::Float32(vec![1.0; 8]),
    };
    let err = w.write_data_block(&block).unwrap_err();
    assert!(matches!(err, mrc::Error::BlockShapeMismatch { .. }));
}

#[test]
fn write_block_as_rejects_wrong_length() {
    let f = TempFile::new("as_wrong_len");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<i16>()
        .finish()
        .unwrap();
    let block = VoxelBlock {
        offset: [0, 0, 0],
        shape: [4, 4, 1],
        data: vec![0.0f32; 8],
    };
    let err = w.write_block_as(&block).unwrap_err();
    assert!(matches!(err, mrc::Error::BlockShapeMismatch { .. }));
}

#[test]
fn write_u4_block_rejects_wrong_length_without_panicking() {
    let f = TempFile::new("u4_wrong_len");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode_raw(Mode::Packed4Bit.as_i32())
        .finish()
        .unwrap();
    let block = VoxelBlock {
        offset: [0, 0, 0],
        shape: [4, 4, 1],
        data: vec![1u8; 4],
    };
    let err = w.write_u4_block(&block).unwrap_err();
    assert!(matches!(err, mrc::Error::BlockShapeMismatch { .. }));
}

#[test]
fn write_u8_block_rejects_wrong_length() {
    let f = TempFile::new("u8_wrong_len");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<u16>()
        .finish()
        .unwrap();
    let block = VoxelBlock {
        offset: [0, 0, 0],
        shape: [4, 4, 1],
        data: vec![1u8; 4],
    };
    let err = w.write_u8_block(&block).unwrap_err();
    assert!(matches!(err, mrc::Error::BlockShapeMismatch { .. }));
}

#[test]
fn float_to_integer_conversion_rounds_consistently() {
    let f = TempFile::new("rounding");
    let values = vec![
        1.5f32, 2.5, 3.5, -1.5, -2.5, -3.5, 1.9, -1.9, 1.1, -1.1, 0.0, 7.0, 8.0, 9.0, 10.0, 11.0,
    ];
    {
        let mut w = mrc::create(f.path())
            .shape([16, 1, 1])
            .mode::<i16>()
            .finish()
            .unwrap();
        let block = VoxelBlock::new([0, 0, 0], [16, 1, 1], values.clone()).unwrap();
        w.write_block_as(&block).unwrap();
        w.finalize().unwrap();
    }
    let reader = Reader::open(f.path()).unwrap();
    let block = reader.subregion([0, 0, 0], [16, 1, 1]).unwrap();
    let DataView::Int16(got) = block.data() else {
        panic!("expected Int16");
    };
    let expected: Vec<i16> = values.iter().map(|&v| v.round_ties_even() as i16).collect();
    assert_eq!(got, expected.as_slice());
}

#[test]
fn from_bytes_keeps_the_callers_buffer() {
    let mut h = base_header();
    h.mode = Mode::Float32.as_i32();
    let mut bytes = header_bytes(h).to_vec();
    bytes.extend(std::iter::repeat_n(0u8, 64));
    let before = bytes[1024..].as_ptr() as usize;

    let reader = Reader::from_bytes(bytes).unwrap();
    let after = reader.raw_bytes().as_ptr() as usize;
    assert_eq!(before, after, "from_bytes copied the voxel payload");
}

#[test]
fn finish_buffer_output_can_be_retrieved() {
    let mut w = mrc::create("ignored")
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish_buffer()
        .unwrap();
    w.write_data_block(&DataBlock::Owned {
        offset: [0, 0, 0],
        shape: [4, 4, 1],
        data: OwnedData::Float32(vec![2.0; 16]),
    })
    .unwrap();
    w.update_header_stats().unwrap();
    w.finalize().unwrap();

    let bytes = w.into_buffer().unwrap();
    assert_eq!(bytes.len(), 1024 + 64);
    let reader = Reader::from_bytes(bytes).unwrap();
    let block = reader.read_volume().unwrap();
    let DataView::Float32(data) = block.data() else {
        panic!("expected Float32");
    };
    assert_eq!(data, &[2.0f32; 16]);
}

#[test]
fn into_buffer_rejects_file_writer() {
    let f = TempFile::new("not_memory");
    let w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    let err = w.into_buffer().unwrap_err();
    assert!(matches!(err, mrc::Error::NotInMemory));
}

#[test]
fn from_reader_limit_is_enforced() {
    let bytes = {
        let mut h = base_header();
        h.mode = Mode::Float32.as_i32();
        let mut v = header_bytes(h).to_vec();
        v.extend(std::iter::repeat_n(0u8, 64));
        v
    };
    assert!(Reader::from_reader_with_limit(std::io::Cursor::new(&bytes), 1024).is_err());
    assert!(Reader::from_reader_with_limit(std::io::Cursor::new(&bytes), 4096).is_ok());
}

#[test]
fn strict_open_reports_file_size_mismatch() {
    let f = TempFile::new("short_file");
    let mut h = base_header();
    h.mode = Mode::Float32.as_i32();
    let mut bytes = header_bytes(h).to_vec();
    bytes.extend(std::iter::repeat_n(0u8, 8)); // 8 of 64 declared bytes
    std::fs::write(f.path(), &bytes).unwrap();

    let err = Reader::open(f.path()).unwrap_err();
    assert!(
        matches!(err, mrc::Error::FileSizeMismatch { .. }),
        "expected FileSizeMismatch, got {err:?}"
    );
}

#[test]
fn invalid_header_does_not_truncate_existing_file() {
    let f = TempFile::new("keep_existing");
    std::fs::write(f.path(), b"sentinel").unwrap();

    let result = mrc::create(f.path())
        .shape([0, 0, 0])
        .mode::<f32>()
        .finish();
    assert!(result.is_err());
    assert_eq!(std::fs::read(f.path()).unwrap(), b"sentinel");
}

/// A truncated Packed4Bit file must produce a validation error, not a panic
/// from the nibble unpacker.
#[test]
fn truncated_packed4bit_stats_do_not_panic() {
    let mut h = base_header();
    h.mode = Mode::Packed4Bit.as_i32();
    let mut bytes = header_bytes(h).to_vec();
    bytes.extend([0u8; 2]); // expected 8 packed bytes for 4x4
    let (reader, _) = Reader::from_bytes_permissive(bytes).unwrap();
    let report = mrc::validate_reader(&reader, "truncated", "plain", &[]).unwrap();
    assert!(!report.is_valid());
}

#[test]
fn header_layout_mutation_is_rejected() {
    let f = TempFile::new("layout_mutation");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    w.header_mut().nx = 8;

    let err = w.update_header_stats().unwrap_err();
    assert!(matches!(err, mrc::Error::WriterHeaderChanged));
    let err = w.finalize().unwrap_err();
    assert!(matches!(err, mrc::Error::WriterHeaderChanged));
}

#[test]
fn full_width_partial_height_float_writes_roundtrip() {
    let f = TempFile::new("partial_y_f32");
    let (nx, ny, nz) = (8, 8, 2);
    let data: Vec<f32> = (0..nx * ny * nz).map(|i| i as f32).collect();
    {
        let mut w = mrc::create(f.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        for z in 0..nz {
            for half in 0..2 {
                let y0 = half * 4;
                let start = (z * ny + y0) * nx;
                w.write_data_block(&DataBlock::Owned {
                    offset: [0, y0, z],
                    shape: [nx, 4, 1],
                    data: OwnedData::Float32(data[start..start + 4 * nx].to_vec()),
                })
                .unwrap();
            }
        }
        w.finalize().unwrap();
    }
    let reader = Reader::open(f.path()).unwrap();
    let block = reader.read_volume().unwrap();
    let DataView::Float32(got) = block.data() else {
        panic!("expected Float32");
    };
    assert_eq!(got, data.as_slice());
}

#[test]
fn full_width_partial_height_u8_write_roundtrip() {
    let (nx, ny) = (8, 4);
    let mut w = mrc::create("ignored")
        .shape([nx, ny, 1])
        .mode::<u16>()
        .finish_buffer()
        .unwrap();
    let block = VoxelBlock::new([0, 1, 0], [nx, 2, 1], vec![7u8; nx * 2]).unwrap();
    w.write_u8_block(&block).unwrap();
    w.finalize().unwrap();
    let bytes = w.into_buffer().unwrap();

    let reader = Reader::from_bytes(bytes).unwrap();
    let block = reader.read_volume().unwrap();
    let DataView::Uint16(got) = block.data() else {
        panic!("expected Uint16");
    };
    for y in 0..ny {
        let expected = if (1..3).contains(&y) { 7 } else { 0 };
        assert!(got[y * nx..(y + 1) * nx].iter().all(|&v| v == expected));
    }
}

#[test]
fn full_width_partial_height_packed_odd_width_roundtrip() {
    let (nx, ny) = (5, 4);
    let mut w = mrc::create("ignored")
        .shape([nx, ny, 1])
        .mode_raw(Mode::Packed4Bit.as_i32())
        .finish_buffer()
        .unwrap();
    let values: Vec<u8> = (1..=10).collect();
    let block = VoxelBlock::new([0, 1, 0], [nx, 2, 1], values).unwrap();
    w.write_u4_block(&block).unwrap();
    w.finalize().unwrap();
    let bytes = w.into_buffer().unwrap();

    let reader = Reader::from_bytes(bytes).unwrap();
    let block = reader.read_volume_u8().unwrap();
    for y in 0..ny {
        for x in 0..nx {
            let expected = if (1..3).contains(&y) {
                (1 + (y - 1) * nx + x) as u8
            } else {
                0
            };
            assert_eq!(block.data[y * nx + x], expected, "y={y} x={x}");
        }
    }
}

#[test]
fn finalize_rejects_incomplete_plain_file() {
    let f = TempFile::new("incomplete");
    let mut w = mrc::create(f.path())
        .shape([4, 4, 1])
        .mode::<f32>()
        .finish()
        .unwrap();
    // Write only half of the declared rows.
    w.write_data_block(&DataBlock::Owned {
        offset: [0, 0, 0],
        shape: [4, 2, 1],
        data: OwnedData::Float32(vec![0.0; 8]),
    })
    .unwrap();
    let err = w.finalize().unwrap_err();
    assert!(matches!(err, mrc::Error::FileSizeMismatch { .. }));
}

#[test]
fn imod_unsigned_mode0_conversion_is_consistent() {
    let mut h = base_header();
    h.nx = 3;
    h.ny = 1;
    h.mx = 3;
    h.my = 1;
    h.mode = Mode::Int8.as_i32();
    h.extra[56..60].copy_from_slice(b"IMOD");
    h.extra[60] = 0; // unsigned Mode 0

    let mut bytes = header_bytes(h).to_vec();
    bytes.extend([0u8, 128, 255]);
    let reader = Reader::from_bytes(bytes).unwrap();

    let as_f32 = reader.convert::<f32>().read_volume().unwrap();
    assert_eq!(as_f32.data, vec![0.0, 128.0, 255.0]);

    let as_i16 = reader.convert::<i16>().read_volume().unwrap();
    assert_eq!(as_i16.data, vec![0, 128, 255]);

    let as_u16 = reader.convert::<u16>().read_volume().unwrap();
    assert_eq!(as_u16.data, vec![0, 128, 255]);
}

fn imod_unsigned_header() -> Header {
    let mut h = base_header();
    h.nx = 3;
    h.ny = 1;
    h.mx = 3;
    h.my = 1;
    h.mode = Mode::Int8.as_i32();
    h.extra[56..60].copy_from_slice(b"IMOD");
    h.extra[60] = 0; // unsigned Mode 0
    h
}

#[test]
fn imod_unsigned_mode0_stats_validate_as_unsigned() {
    let mut h = imod_unsigned_header();
    let mean: f64 = (0.0 + 128.0 + 255.0) / 3.0;
    let var = ((0.0 - mean).powi(2) + (128.0 - mean).powi(2) + (255.0 - mean).powi(2)) / 3.0;
    h.dmin = 0.0;
    h.dmax = 255.0;
    h.dmean = mean as f32;
    h.rms = (var.sqrt()) as f32;

    let mut bytes = header_bytes(h).to_vec();
    bytes.extend([0u8, 128, 255]);
    let reader = Reader::from_bytes(bytes).unwrap();
    reader.validate_header_stats().unwrap();
}

#[test]
fn writer_stats_honor_imod_unsigned_mode0() {
    let h = imod_unsigned_header();
    let mut w = mrc::Writer::from_writer(std::io::Cursor::new(Vec::new()), h, &[]).unwrap();
    w.write_data_block(&DataBlock::Owned {
        offset: [0, 0, 0],
        shape: [3, 1, 1],
        data: OwnedData::Int8(vec![0, -128, -1]),
    })
    .unwrap();
    w.update_header_stats().unwrap();
    assert_eq!(w.header().dmin, 0.0);
    assert_eq!(w.header().dmax, 255.0);
}

#[test]
fn permissive_from_bytes_ignores_trailing_bytes() {
    let mut h = base_header();
    h.mode = Mode::Float32.as_i32();
    let mut bytes = header_bytes(h).to_vec();
    bytes.extend(std::iter::repeat_n(0u8, 64));
    bytes.extend([9u8; 4]);

    let (reader, _) = Reader::from_bytes_permissive(bytes).unwrap();
    assert!(!reader.is_truncated());
    assert_eq!(reader.raw_bytes().len(), 64);
    reader.validate_header_stats().unwrap();
}
