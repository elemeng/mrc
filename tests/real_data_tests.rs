//! Tests on real MRC data files from `real_data/`.
//!
//! Covers every public API path applicable to Mode 1 (Int16) and Mode 2 (Float32)
//! files. Large-file operations (tomogram) use subregions/slices only.
//!
//! Run with: `cargo test --all-features --test real_data`

use mrc::*;
use mrc::{DataBlock, OwnedData};
use std::path::PathBuf;

fn real_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("real_data")
}

fn path(name: &str) -> PathBuf {
    real_dir().join(name)
}

const SEARCH: &str = "search.mrc";
const ATLAS: &str = "Atlas.mrc";
const PREVIEW: &str = "preview.mrc";
const BOX320: &str = "box320_bin2_mask.mrc";
const EMMAP: &str = "em-map.mrc";
const TOMO: &str = "tomogram.mrc";

/// Return all real data files that exist on disk (the directory is gitignored).
fn available_files() -> Vec<&'static str> {
    [SEARCH, ATLAS, PREVIEW, BOX320, EMMAP, TOMO]
        .into_iter()
        .filter(|name| path(name).exists())
        .collect()
}

#[track_caller]
fn assert_open_ok(name: &str) -> Reader {
    let p = path(name);
    if !p.exists() {
        panic!(
            "real data file not found: {}\n\
             The real_data/ directory is gitignored (files are too large for GitHub).\n\
             Place any MRC files here (from EMDB, EMPIAR, or your own microscope)\n\
             and the tests will exercise every API path against them.",
            p.display()
        );
    }
    match Reader::open(p) {
        Ok(r) => r,
        Err(e) => panic!("failed to open {name}: {e}"),
    }
}

// ============================================================================
// Header validation on all files
// ============================================================================

#[test]
fn header_validate_detailed_all() {
    for name in available_files() {
        let r = assert_open_ok(name);
        assert!(
            r.header().validate_detailed().is_ok(),
            "{name}: header validation failed"
        );
    }
}

#[test]
fn header_validate_bool_all() {
    for name in available_files() {
        let r = assert_open_ok(name);
        assert!(r.header().validate(), "{name}: validate() returned false");
    }
}

#[test]
fn header_validate_permissive_all() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let result = r.header().validate_permissive();
        assert!(
            result.is_ok(),
            "{name}: permissive validate failed: {:?}",
            result
        );
    }
}

// ============================================================================
// Header property tests
// ============================================================================

#[test]
fn header_dimensions_consistent() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        assert!(h.nx > 0, "{name}: nx={}", h.nx);
        assert!(h.ny > 0, "{name}: ny={}", h.ny);
        assert!(h.nz > 0, "{name}: nz={}", h.nz);
        let s = r.shape();
        assert_eq!(s.nx as i32, h.nx, "{name}: shape.nx != header.nx");
        assert_eq!(s.ny as i32, h.ny, "{name}: shape.ny != header.ny");
        assert_eq!(s.nz as i32, h.nz, "{name}: shape.nz != header.nz");
    }
}

#[test]
fn header_mode_consistent() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let m = r.mode();
        assert_eq!(m.as_i32(), r.header().mode, "{name}: mode mismatch");
        assert!(
            matches!(m, Mode::Int16 | Mode::Float32),
            "{name}: unexpected mode {m:?}"
        );
    }
}

#[test]
fn header_detect_endian() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        let f = FileEndian::from_machst(&h.machst);
        assert_eq!(h.detect_endian(), f, "{name}: detect_endian mismatch");
        assert_eq!(
            r.endian(),
            h.detect_endian(),
            "{name}: reader.endian != header endian"
        );
    }
}

#[test]
fn header_cell_properties() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        let [xl, yl, zl] = h.cell_lengths();
        assert_eq!(xl, h.xlen, "{name}: cell_lengths x");
        assert_eq!(yl, h.ylen, "{name}: cell_lengths y");
        assert_eq!(zl, h.zlen, "{name}: cell_lengths z");
        let [a, b, g] = h.cell_angles();
        assert_eq!(a, h.alpha, "{name}: cell_angles alpha");
        assert_eq!(b, h.beta, "{name}: cell_angles beta");
        assert_eq!(g, h.gamma, "{name}: cell_angles gamma");
        let vol = h.cell_volume();
        assert!(vol >= 0.0, "{name}: negative cell volume");
    }
}

#[test]
fn header_voxel_size_and_sampling() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        let vs = h.voxel_size();
        assert!(
            vs.iter().all(|&v| v >= 0.0 || v.is_finite()),
            "{name}: invalid voxel size {vs:?}"
        );
        let s = h.sampling();
        assert_eq!(s, [h.mx, h.my, h.mz], "{name}: sampling mismatch");
    }
}

#[test]
fn header_density_stats() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        let (dmin, dmax, _dmean, _rms) = h.density_stats();
        assert_eq!(dmin, h.dmin, "{name}: dmin");
        assert!(dmin <= dmax, "{name}: dmin > dmax");
    }
}

#[test]
fn header_labels_and_properties() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        // exttyp
        let et = h.exttyp();
        let _ets = h.exttyp_str();
        assert_eq!(et.len(), 4, "{name}: exttyp wrong len");
        // nversion
        let _ = h.nversion();
        // labels
        let labels = h.get_labels();
        assert!(
            labels.len() <= h.nlabl as usize,
            "{name}: more labels than nlabl"
        );
        if h.nlabl > 0 {
            if let Some(l0) = h.label_at(0) {
                assert!(!l0.is_empty(), "{name}: label at 0 is empty");
            }
        }
        // is_standard_map
        let _ = h.is_standard_map();
        // detect_imod
        let _ = h.detect_imod();
        let _ = h.is_y_inverted();
        // data offset/size
        let _ = h.data_offset();
        let _ = h.data_size();
        // nstart
        let _ = h.nstart();
        // logical shape
        let _ = h.logical_shape();
    }
}

#[test]
fn header_volume_type_methods() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        // Should be one of: image, image_stack, volume, volume_stack
        let is_si = h.is_single_image();
        let is_is = h.is_image_stack();
        let is_v = h.is_volume();
        let is_vs = h.is_volume_stack();
        let count = is_si as u8 + is_is as u8 + is_v as u8 + is_vs as u8;
        assert_eq!(
            count, 1,
            "{name}: exactly one volume type expected, got si={is_si} is={is_is} v={is_v} vs={is_vs}"
        );
        // Same on reader
        assert_eq!(r.is_single_image(), is_si);
        assert_eq!(r.is_image_stack(), is_is);
        assert_eq!(r.is_volume(), is_v);
        assert_eq!(r.is_volume_stack(), is_vs);
    }
}

#[test]
fn header_decode_encode_roundtrip() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        let mut buf = [0u8; 1024];
        h.encode_to_bytes(&mut buf);
        let decoded = Header::decode_from_bytes(&buf);
        assert_eq!(decoded.nx, h.nx, "{name}: decode nx mismatch");
        assert_eq!(decoded.mode, h.mode, "{name}: decode mode mismatch");
        assert_eq!(decoded.map, h.map, "{name}: decode map mismatch");
    }
}

#[test]
fn header_decode_from_bytes_with_info() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let h = r.header();
        let mut buf = [0u8; 1024];
        h.encode_to_bytes(&mut buf);
        let (decoded, _warn) = Header::decode_from_bytes_with_info(&buf);
        assert_eq!(decoded.nx, h.nx);
        assert_eq!(decoded.ny, h.ny);
        assert_eq!(decoded.nz, h.nz);
    }
}

// ============================================================================
// Top-level function tests
// ============================================================================

#[test]
fn top_level_open() {
    let r = open(path(SEARCH)).unwrap();
    assert!(r.shape().nx > 0);
}

#[test]
fn top_level_read_as_f32() {
    let (h, data): (_, Vec<f32>) = read_as(path(SEARCH)).unwrap();
    assert_eq!(data.len(), (h.nx * h.ny * h.nz) as usize);
}

#[test]
fn top_level_read_as_i16() {
    let (h, data): (_, Vec<i16>) = read_as(path(SEARCH)).unwrap();
    assert_eq!(data.len(), (h.nx * h.ny * h.nz) as usize);
}

#[test]
fn top_level_read_as_f32_float_file() {
    let (h, data): (_, Vec<f32>) = read_as(path(BOX320)).unwrap();
    assert_eq!(data.len(), (h.nx * h.ny * h.nz) as usize);
}

// ============================================================================
// Validation tests
// ============================================================================

#[test]
fn validate_full_strict() {
    for name in [SEARCH, EMMAP] {
        let report = validate_full(path(name), false).unwrap();
        assert!(
            report.is_valid(),
            "{name}: strict validation failed: {:?}",
            report.issues
        );
    }
    // box320 has known RMS stats mismatch in header (claimed=0, actual=0.365)
    let report = validate_full(path(BOX320), false).unwrap();
    assert!(
        !report.is_valid(),
        "box320: should fail strict validation due to RMS mismatch"
    );
    let errors: Vec<_> = report.by_severity(Severity::Error).collect();
    assert_eq!(
        errors.len(),
        1,
        "box320: expected 1 error, got {:?}",
        errors
    );
    assert!(
        errors[0].message.contains("Mismatch"),
        "box320: expected stats mismatch"
    );
}

#[test]
fn validate_full_permissive() {
    for name in [SEARCH, EMMAP] {
        let report = validate_full(path(name), true).unwrap();
        assert!(
            report.is_valid(),
            "{name}: permissive validation failed: {:?}",
            report.issues
        );
    }
    // box320 has known RMS stats mismatch
    let report = validate_full(path(BOX320), true).unwrap();
    assert!(
        !report.is_valid(),
        "box320: should still fail permissive due to RMS"
    );
    assert!(report.path.ends_with(BOX320), "wrong path in report");
    assert!(
        report.path.ends_with("box320_bin2_mask.mrc"),
        "wrong path in report"
    );
}

#[test]
fn validate_reader_test() {
    let r = assert_open_ok(SEARCH);
    let report = validate_reader(&r, path(SEARCH).to_str().unwrap(), "plain", &[]).unwrap();
    assert!(report.is_valid());
    // Test by_severity iterator
    let warnings: Vec<_> = report.by_severity(Severity::Warning).collect();
    let errors: Vec<_> = report.by_severity(Severity::Error).collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    let _ = warnings;
}

// ============================================================================
// Reader method tests
// ============================================================================

#[test]
fn reader_open_permissive() {
    for name in [SEARCH, BOX320, EMMAP] {
        let (r, warnings) = Reader::open_permissive(path(name)).unwrap();
        assert!(r.shape().nx > 0);
        assert!(!r.is_truncated(), "{name}: should not be truncated");
        let _ = warnings;
    }
}

#[test]
fn reader_raw_bytes() {
    let r = assert_open_ok(SEARCH);
    let raw = r.raw_bytes();
    let expected_size = r.header().data_size().unwrap_or(0);
    assert_eq!(raw.len(), expected_size, "raw_bytes wrong size");
}

#[test]
fn reader_ext_header_bytes() {
    for name in available_files() {
        let r = assert_open_ok(name);
        let ext = r.ext_header_bytes();
        let h = r.header();
        assert_eq!(
            ext.len(),
            h.nsymbt as usize,
            "{name}: ext header size mismatch"
        );
    }
}

#[test]
fn reader_read_block_bytes_small() {
    let r = assert_open_ok(SEARCH);
    let bytes = r.read_block_bytes([0, 0, 0], [16, 16, 1]).unwrap();
    // Mode 1 = Int16 = 2 bytes per voxel
    assert_eq!(bytes.len(), 16 * 16 * 2, "read_block_bytes wrong size");
}

// ============================================================================
// Subregion tests
// ============================================================================

#[test]
fn subregion_small_search() {
    let r = assert_open_ok(SEARCH);
    let block = r.subregion([0, 0, 0], [32, 32, 4]).unwrap();
    assert_eq!(block.offset(), [0, 0, 0]);
    assert_eq!(block.shape(), [32, 32, 4]);
    match block.data() {
        DataView::Int16(d) => assert_eq!(d.len(), 32 * 32 * 4, "wrong data len"),
        _ => panic!("expected Int16"),
    }
}

#[test]
fn subregion_offset_search() {
    let r = assert_open_ok(SEARCH);
    let block = r.subregion([100, 50, 5], [64, 64, 3]).unwrap();
    assert_eq!(block.offset(), [100, 50, 5]);
    assert_eq!(block.shape(), [64, 64, 3]);
    match block.data() {
        DataView::Int16(d) => assert_eq!(d.len(), 64 * 64 * 3),
        _ => panic!("expected Int16"),
    }
    // VoxelBlock::is_full_volume — DataBlock doesn't have it, skip for subregion
}

#[test]
fn subregion_float32_box320() {
    let r = assert_open_ok(BOX320);
    let block = r.subregion([16, 16, 16], [32, 32, 32]).unwrap();
    match block.data() {
        DataView::Float32(d) => {
            assert_eq!(d.len(), 32 * 32 * 32);
            // Data should contain only 0.0 or 1.0 (binary mask)
            for &v in d.iter() {
                assert!(
                    (v - 0.0).abs() < 1e-6 || (v - 1.0).abs() < 1e-6,
                    "unexpected value {v} in box320 mask"
                );
            }
        }
        _ => panic!("expected Float32"),
    }
}

#[test]
fn subregion_tomogram() {
    let r = assert_open_ok(TOMO);
    let block = r.subregion([500, 500, 100], [32, 32, 8]).unwrap();
    assert_eq!(block.offset(), [500, 500, 100]);
    assert_eq!(block.shape(), [32, 32, 8]);
    match block.data() {
        DataView::Float32(d) => {
            assert_eq!(d.len(), 32 * 32 * 8);
            assert!(
                d.iter().all(|&v| v.is_finite()),
                "tomogram has NaN/Inf values"
            );
        }
        _ => panic!("expected Float32"),
    }
}

#[test]
fn subregion_bounds_error() {
    let r = assert_open_ok(SEARCH);
    let result = r.subregion([2000, 0, 0], [1, 1, 1]);
    assert!(result.is_err(), "should fail for out-of-bounds subregion");
    match result {
        Err(Error::BoundsError { .. }) => {}
        other => panic!("expected BoundsError, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn subregion_emmap_corner() {
    let r = assert_open_ok(EMMAP);
    // Top-left corner
    let block = r.subregion([0, 0, 0], [8, 8, 8]).unwrap();
    assert_eq!(block.offset(), [0, 0, 0]);
    match block.data() {
        DataView::Float32(d) => assert_eq!(d.len(), 8 * 8 * 8),
        _ => panic!("expected Float32"),
    }
    // Bottom-right corner
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let nz = r.shape().nz;
    let block = r.subregion([nx - 8, ny - 8, nz - 4], [8, 8, 4]).unwrap();
    assert_eq!(block.shape(), [8, 8, 4]);
}

// ============================================================================
// read_volume tests
// ============================================================================

#[test]
fn read_volume_search() {
    let r = assert_open_ok(SEARCH);
    let block = r.read_volume().unwrap();
    assert_eq!(block.offset(), [0, 0, 0]);
    assert_eq!(block.shape(), [r.shape().nx, r.shape().ny, r.shape().nz]);
    match block.data() {
        DataView::Int16(d) => {
            let total = r.shape().nx * r.shape().ny * r.shape().nz;
            assert_eq!(d.len(), total, "volume has wrong voxel count");
        }
        _ => panic!("expected Int16"),
    }
}

#[test]
fn read_volume_emmap() {
    let r = assert_open_ok(EMMAP);
    let block = r.read_volume().unwrap();
    match block.data() {
        DataView::Float32(d) => {
            let total = r.shape().nx * r.shape().ny * r.shape().nz;
            assert_eq!(d.len(), total);
        }
        _ => panic!("expected Float32"),
    }
}

#[test]
fn read_volume_box320() {
    let r = assert_open_ok(BOX320);
    let block = r.read_volume().unwrap();
    match block.data() {
        DataView::Float32(d) => {
            let total = r.shape().nx * r.shape().ny * r.shape().nz;
            assert_eq!(d.len(), total);
        }
        _ => panic!("expected Float32"),
    }
}

// ============================================================================
// Iterator tests: slices
// ============================================================================

#[test]
fn iterate_slices_search() {
    let r = assert_open_ok(SEARCH);
    let nz = r.shape().nz;
    let mut count = 0;
    for s in r.slices() {
        let block = s.unwrap();
        assert_eq!(block.shape()[2], 1, "slice should have sz=1");
        assert_eq!(block.offset()[2], count);
        match block.data() {
            DataView::Int16(d) => assert_eq!(d.len(), r.shape().nx * r.shape().ny),
            _ => panic!("expected Int16"),
        }
        count += 1;
    }
    assert_eq!(count, nz);
}

#[test]
fn iterate_slices_box320() {
    let r = assert_open_ok(BOX320);
    let nz = r.shape().nz;
    let mut count = 0;
    for s in r.slices() {
        let block = s.unwrap();
        match block.data() {
            DataView::Float32(d) => assert!(!d.is_empty()),
            _ => panic!("expected Float32"),
        }
        count += 1;
    }
    assert_eq!(count, nz);
}

#[test]
fn iterate_slices_tomogram_subset() {
    let r = assert_open_ok(TOMO);
    // Only iterate first 5 slices (large file)
    let mut count = 0;
    for s in r.slices().take(5) {
        let block = s.unwrap();
        assert_eq!(block.offset()[2], count);
        count += 1;
    }
    assert_eq!(count, 5);
}

// ============================================================================
// Iterator tests: slabs
// ============================================================================

#[test]
fn iterate_slabs_search() {
    let r = assert_open_ok(SEARCH);
    let k = 3;
    let mut count = 0;
    let mut total_z = 0;
    for s in r.slabs(k) {
        let block = s.unwrap();
        let sz = block.shape()[2];
        assert!(sz <= k, "slab too deep");
        total_z += sz;
        count += 1;
    }
    assert_eq!(total_z, r.shape().nz, "total z from slabs != nz");
    assert!(count > 0);
}

#[test]
fn iterate_slabs_box320_large() {
    let r = assert_open_ok(BOX320);
    let mut count = 0;
    for s in r.slabs(64) {
        let block = s.unwrap();
        match block.data() {
            DataView::Float32(d) => assert!(!d.is_empty()),
            _ => panic!("expected Float32"),
        }
        count += 1;
        if count >= 3 {
            break;
        }
    }
}

// ============================================================================
// Iterator tests: tiles
// ============================================================================

#[test]
fn iterate_tiles_search() {
    let r = assert_open_ok(SEARCH);
    let tile_shape = [128, 128, 2];
    let mut count = 0;
    for tile in r.tiles(tile_shape).unwrap().take(8) {
        let block = tile.unwrap();
        count += 1;
        match block.data() {
            DataView::Int16(d) => assert!(!d.is_empty()),
            _ => panic!("expected Int16"),
        }
    }
    assert_eq!(count, 8);
}

#[test]
fn iterate_tiles_box320() {
    let r = assert_open_ok(BOX320);
    let tile_shape = [80, 80, 80];
    for tile in r.tiles(tile_shape).unwrap().take(4) {
        let block = tile.unwrap();
        match block.data() {
            DataView::Float32(d) => assert!(!d.is_empty()),
            _ => panic!("expected Float32"),
        }
    }
}

#[test]
fn iterate_tiles_tomogram() {
    let r = assert_open_ok(TOMO);
    let tile_shape = [256, 256, 64];
    for tile in r.tiles(tile_shape).unwrap().take(4) {
        let block = tile.unwrap();
        match block.data() {
            DataView::Float32(d) => assert!(!d.is_empty()),
            _ => panic!("expected Float32"),
        }
    }
}

// ============================================================================
// Stats validation tests
// ============================================================================

#[test]
fn validate_header_stats_search() {
    let r = assert_open_ok(SEARCH);
    let result = r.validate_header_stats();
    assert!(
        result.is_ok(),
        "search.mrc: stats validation failed: {result:?}"
    );
}

#[test]
fn validate_header_stats_box320() {
    let r = assert_open_ok(BOX320);
    let result = r.validate_header_stats();
    // box320 has known RMS mismatch (claimed=0, actual=0.365)
    match result {
        Err(Error::StatsMismatch { .. }) => {}
        other => panic!("expected StatsMismatch for box320, got {other:?}"),
    }
}

#[test]
fn validate_header_stats_tomogram() {
    let r = assert_open_ok(TOMO);
    let result = r.validate_header_stats();
    // tomogram has known stats mismatch (dmean, rms differ)
    match result {
        Err(Error::StatsMismatch { .. }) => {}
        other => panic!("expected StatsMismatch for tomogram, got {other:?}"),
    }
}

// ============================================================================
// ConvertReader tests
// ============================================================================

#[test]
fn convert_search_to_f32_full() {
    let r = assert_open_ok(SEARCH);
    let block = r.convert::<f32>().read_volume().unwrap();
    let total = r.shape().nx * r.shape().ny * r.shape().nz;
    assert_eq!(block.data.len(), total);
    assert_eq!(block.offset, [0, 0, 0]);
    assert_eq!(block.shape, [r.shape().nx, r.shape().ny, r.shape().nz]);
    // All values should be finite
    for v in &block.data {
        assert!(v.is_finite(), "NaN/Inf in converted data");
    }
}

#[test]
fn convert_box320_to_i16() {
    let r = assert_open_ok(BOX320);
    let block = r.convert::<i16>().read_volume().unwrap();
    let total = r.shape().nx * r.shape().ny * r.shape().nz;
    assert_eq!(block.data.len(), total);
    // Values should be 0 or 1 in binary mask
    for &v in &block.data {
        assert!(
            v == 0 || v == 1,
            "unexpected i16 value {v} from box320 mask"
        );
    }
}

#[test]
fn convert_slices_f32_search() {
    let r = assert_open_ok(SEARCH);
    for slice in r.convert::<f32>().slices().take(3) {
        let block = slice.unwrap();
        assert_eq!(block.data.len(), r.shape().nx * r.shape().ny);
    }
}

#[test]
fn convert_slabs_f32_box320() {
    let r = assert_open_ok(BOX320);
    for slab in r.convert::<f32>().slabs(32).take(4) {
        let block = slab.unwrap();
        assert!(!block.data.is_empty());
        for v in &block.data {
            assert!(v.is_finite());
        }
    }
}

#[test]
fn convert_tiles_f32_emmap() {
    let r = assert_open_ok(EMMAP);
    for tile in r.convert::<f32>().tiles([64, 64, 64]).unwrap().take(4) {
        let block = tile.unwrap();
        assert!(!block.data.is_empty());
    }
}

#[test]
fn convert_subregion_f32_search() {
    let r = assert_open_ok(SEARCH);
    let block = r
        .convert::<f32>()
        .subregion([50, 50, 2], [128, 128, 4])
        .unwrap();
    assert_eq!(block.offset, [50, 50, 2]);
    assert_eq!(block.shape, [128, 128, 4]);
    assert_eq!(block.data.len(), 128 * 128 * 4);
}

#[test]
fn convert_subregion_f32_box320() {
    let r = assert_open_ok(BOX320);
    let block = r
        .convert::<f32>()
        .subregion([32, 32, 32], [64, 64, 64])
        .unwrap();
    assert_eq!(block.data.len(), 64 * 64 * 64);
}

#[test]
fn convert_slices_tomogram() {
    let r = assert_open_ok(TOMO);
    for slice in r.convert::<f32>().slices().take(3) {
        let block = slice.unwrap();
        assert_eq!(block.data.len(), r.shape().nx * r.shape().ny);
    }
}

// ============================================================================
// ConvertReader: type variations (Int16 file)
// ============================================================================

#[test]
fn convert_search_to_multiple_types() {
    let r = assert_open_ok(SEARCH);
    // f32
    let _ = r.convert::<f32>().read_volume().unwrap();
    // i16 (same type)
    let _ = r.convert::<i16>().read_volume().unwrap();
}

#[test]
fn convert_with_m0_signed() {
    let r = assert_open_ok(SEARCH);
    let block = r
        .convert::<f32>()
        .with_m0_interpretation(M0Interpretation::Signed)
        .read_volume()
        .unwrap();
    assert!(!block.data.is_empty());
}

#[test]
fn convert_with_complex_strategy() {
    let r = assert_open_ok(SEARCH);
    let block = r
        .convert::<f32>()
        .with_complex_strategy(ComplexToRealStrategy::RealPart)
        .read_volume()
        .unwrap();
    assert!(!block.data.is_empty());
}

// ============================================================================
// VoxelBlock methods
// ============================================================================

#[test]
fn voxelblock_properties() {
    let r = assert_open_ok(SEARCH);
    let block = r
        .convert::<f32>()
        .subregion([0, 0, 0], [16, 16, 4])
        .unwrap();
    assert_eq!(block.len(), 16 * 16 * 4);
    assert!(!block.is_empty());
    // is_full_volume — false for subregion
    assert!(!block.is_full_volume(&r.shape()));
}

// ============================================================================
// DataBlock methods
// ============================================================================

#[test]
fn datablock_borrowed_variants() {
    let r = assert_open_ok(BOX320);
    // First, ensure DataBlock works with Owned variant too
    let block = r.read_volume().unwrap();
    let off = block.offset();
    let sh = block.shape();
    assert_eq!(off, [0, 0, 0]);
    assert_eq!(sh[0], r.shape().nx);
    let dv = block.data();
    match dv {
        DataView::Float32(_) => {}
        _ => panic!("wrong DataView variant"),
    }
}

#[test]
fn datablock_owned_variant() {
    let r = assert_open_ok(SEARCH);
    let block = r.read_volume().unwrap();
    match &block {
        DataBlock::Borrowed { .. } => {}
        DataBlock::Owned { .. } => {}
        _ => {}
    }
}

// ============================================================================
// Mode enum tests
// ============================================================================

#[test]
fn mode_methods() {
    let r = assert_open_ok(SEARCH);
    let m = r.mode();
    assert_eq!(m, Mode::Int16);
    assert!(m.is_integer());
    assert!(!m.is_float());
    assert!(!m.is_complex());
    assert_eq!(m.byte_size(), 2);
    assert_eq!(m.byte_size_for_count(100), 200);
    let m2 = Mode::from_i32(1);
    assert_eq!(m2, Some(Mode::Int16));

    let r2 = assert_open_ok(BOX320);
    let mf = r2.mode();
    assert_eq!(mf, Mode::Float32);
    assert!(mf.is_float());
    assert!(!mf.is_integer());
}

// ============================================================================
// FileEndian tests
// ============================================================================

#[test]
fn file_endian_native_vs_file() {
    let r = assert_open_ok(SEARCH);
    let fe = r.endian();
    let _native = FileEndian::native();
    // All files are LE
    assert_eq!(fe, FileEndian::LittleEndian);
    let _ = fe.is_native();
    let _ = fe.opposite();
    let machst = fe.to_machst();
    let roundtrip = FileEndian::from_machst(&machst);
    assert_eq!(roundtrip, fe);
}

// ============================================================================
// VolumeShape tests
// ============================================================================

#[test]
fn volume_shape_operations() {
    let r = assert_open_ok(SEARCH);
    let s = r.shape();
    assert!(!s.is_empty());
    let tv = s.total_voxels();
    assert_eq!(tv, Some(s.nx * s.ny * s.nz));
    // from_header
    let s2 = VolumeShape::from_header(r.header()).unwrap();
    assert_eq!(s.nx, s2.nx);
    // contains_block
    assert!(s.contains_block([0, 0, 0], [1, 1, 1]));
    assert!(!s.contains_block([10000, 0, 0], [1, 1, 1]));
    // checked_linear_index
    assert_eq!(s.checked_linear_index([0, 0, 0]), Some(0));
}

// ============================================================================
// Extended header tests
// ============================================================================

#[test]
fn extended_header_on_available_files() {
    for name in available_files() {
        let r = assert_open_ok(name);
        // parse_extended_header
        let parsed = r.parse_extended_header();
        match parsed {
            ExtHeaderData::None => {
                assert_eq!(
                    r.header().nsymbt,
                    0,
                    "{name}: nsymbt > 0 but ExtHeaderData::None"
                );
            }
            _ => {
                assert!(
                    r.header().nsymbt > 0,
                    "{name}: nsymbt == 0 but extended header parsed"
                );
            }
        }
    }
}

#[test]
fn extended_header_convenience_methods() {
    for name in [SEARCH, BOX320] {
        let r = assert_open_ok(name);
        let _ = r.fei1_metadata();
        let _ = r.fei2_metadata();
        let _ = r.ccp4_records();
        let _ = r.mrco_records();
        let _ = r.seri_records();
        let _ = r.agar_records();
        let _ = r.imod_metadata();
    }
}

// ============================================================================
// Mode variants tests on DataView
// ============================================================================

#[test]
fn dataview_type_idents() {
    // Mode 1 (Int16) file
    let r = assert_open_ok(SEARCH);
    let block = r.read_volume().unwrap();
    assert!(
        matches!(block.data(), DataView::Int16(_)),
        "search should be Int16"
    );

    // Mode 2 (Float32) file
    let r = assert_open_ok(BOX320);
    let block = r.read_volume().unwrap();
    assert!(
        matches!(block.data(), DataView::Float32(_)),
        "box320 should be Float32"
    );
}

// ============================================================================
// OwnedData tests
// ============================================================================

#[test]
fn owned_data_display() {
    let d = OwnedData::Float32(vec![1.0, 2.0]);
    assert!(format!("{d:?}").contains("Float32"));

    let d = OwnedData::Int16(vec![1, 2]);
    assert!(format!("{d:?}").contains("Int16"));
}

// ============================================================================
// ComplexToRealStrategy and M0Interpretation tests
// ============================================================================

#[test]
fn strategy_and_interpretation() {
    // Just verify these types can be constructed
    let _ = ComplexToRealStrategy::RealPart;
    let _ = ComplexToRealStrategy::ImaginaryPart;
    let _ = ComplexToRealStrategy::Magnitude;
    let _ = ComplexToRealStrategy::Phase;
    let _ = M0Interpretation::Signed;
    let _ = M0Interpretation::Unsigned;
}

// ============================================================================
// Multiple volumes on same file (idempotency)
// ============================================================================

#[test]
fn repeated_read_volume_idempotent() {
    let r = assert_open_ok(SEARCH);
    let block1 = r.read_volume().unwrap();
    let block2 = r.read_volume().unwrap();
    match (block1.data(), block2.data()) {
        (DataView::Int16(a), DataView::Int16(b)) => assert_eq!(a, b),
        _ => panic!(),
    }
}

// ============================================================================
// Cross-file consistency checks
// ============================================================================

#[test]
fn emmap_box320_same_dimensions() {
    let r1 = assert_open_ok(EMMAP);
    let r2 = assert_open_ok(BOX320);
    assert_eq!(r1.shape().nx, r2.shape().nx);
    assert_eq!(r1.shape().ny, r2.shape().ny);
    assert_eq!(r1.shape().nz, r2.shape().nz);
}

// ============================================================================
// Mmaps tests
// ============================================================================

#[cfg(feature = "mmap")]
#[test]
fn mmap_read_subregion() {
    let r = assert_open_ok(SEARCH);
    let block = r.subregion([0, 0, 0], [32, 32, 4]).unwrap();
    match block {
        DataBlock::Borrowed { .. } => { /* ok, mmap may give Borrowed */ }
        DataBlock::Owned { .. } => { /* ok, some modes give Owned */ }
        _ => {} /* future variants */
    }
}

#[cfg(feature = "mmap")]
#[test]
fn mmap_read_volume_box320() {
    let r = assert_open_ok(BOX320);
    let block = r.read_volume().unwrap();
    assert!(matches!(block.data(), DataView::Float32(_)));
}

// ============================================================================
// Writer roundtrip tests
// ============================================================================

struct TempMrc(std::path::PathBuf);

impl TempMrc {
    fn new(suffix: &str) -> Self {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "mrc_real_writer_{}_{}.mrc",
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

#[test]
fn writer_roundtrip_search_as_f32() {
    // Read Int16 file, write as Float32, read back, verify dimensions and data
    let r = assert_open_ok(SEARCH);
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let nz = r.shape().nz;
    let data_f32: Vec<f32> = r.convert::<f32>().read_volume().unwrap().data;

    let out = TempMrc::new("search_f32");
    {
        let mut w = create(out.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [nx, ny, nz],
            data: OwnedData::Float32(data_f32.clone()),
        })
        .unwrap();
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    assert_eq!(r2.shape().nx, nx);
    assert_eq!(r2.shape().ny, ny);
    assert_eq!(r2.shape().nz, nz);
    assert_eq!(r2.mode(), Mode::Float32);
    let block = r2.read_volume().unwrap();
    let DataView::Float32(back) = block.data() else {
        panic!("expected Float32")
    };
    // Verify header stats are accurate
    assert!(
        r2.validate_header_stats().is_ok(),
        "stats mismatch on written file"
    );
    // Verify data matches (within f32 precision from i16)
    for (a, b) in data_f32.iter().zip(back.iter()) {
        assert!((a - b).abs() < 1.0, "data mismatch: {a} vs {b}");
    }
}

#[test]
fn writer_roundtrip_box320() {
    // Read Float32 file, write as Float32, read back byte-for-byte
    let r = assert_open_ok(BOX320);
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let nz = r.shape().nz;
    let block = r.read_volume().unwrap();
    let DataView::Float32(original) = block.data() else {
        panic!("expected Float32")
    };

    let out = TempMrc::new("box320_f32");
    {
        let mut w = create(out.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [nx, ny, nz],
            data: OwnedData::Float32(original.to_vec()),
        })
        .unwrap();
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    let block2 = r2.read_volume().unwrap();
    let DataView::Float32(back) = block2.data() else {
        panic!("expected Float32")
    };
    assert_eq!(original, back, "data mismatch on box320 roundtrip");
    assert!(r2.validate_header_stats().is_ok(), "stats mismatch");
}

#[test]
fn writer_streaming_slice_by_slice() {
    // Read slices from search.mrc, write them one by one
    let r = assert_open_ok(SEARCH);
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let nz = r.shape().nz;

    let out = TempMrc::new("stream");
    {
        let mut w = create(out.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        for slice in r.convert::<f32>().slices() {
            let s = slice.unwrap();
            w.write_data_block(&DataBlock::Owned {
                offset: [0, 0, s.offset[2]],
                shape: [nx, ny, 1],
                data: OwnedData::Float32(s.data),
            })
            .unwrap();
        }
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    assert_eq!(r2.shape().nz, nz);
    assert_eq!(r2.mode(), Mode::Float32);
    assert!(
        r2.validate_header_stats().is_ok(),
        "stats mismatch on streaming write"
    );
}

#[test]
fn writer_single_slice_data() {
    // Read one slice from search.mrc, write as a single-slice file
    let r = assert_open_ok(SEARCH);
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let slice = r.subregion([0, 0, 0], [nx, ny, 1]).unwrap();
    let DataView::Int16(data) = slice.data() else {
        panic!("expected Int16")
    };

    let out = TempMrc::new("single");
    {
        let mut w = create(out.path())
            .shape([nx, ny, 1])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [nx, ny, 1],
            data: OwnedData::Int16(data.to_vec()),
        })
        .unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    assert_eq!(r2.shape().nx, nx);
    assert_eq!(r2.shape().ny, ny);
    assert_eq!(r2.shape().nz, 1);
    assert!(r2.is_single_image(), "should be a single image");
}

#[cfg(feature = "gzip")]
#[test]
fn writer_roundtrip_gzip() {
    // Write a small subregion as gzip, read back
    let r = assert_open_ok(SEARCH);
    let block = r
        .convert::<f32>()
        .subregion([0, 0, 0], [32, 32, 4])
        .unwrap();

    let out = TempMrc::new("real_gzip");
    {
        let mut w = create(out.path())
            .shape([32, 32, 4])
            .mode::<f32>()
            .finish_gzip()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [32, 32, 4],
            data: OwnedData::Float32(block.data),
        })
        .unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    assert_eq!(r2.shape().nz, 4);
    assert!(r2.validate_header_stats().is_ok());
}

#[test]
fn writer_set_data_roundtrip() {
    // Use set_data convenience
    let r = assert_open_ok(EMMAP);
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let nz = r.shape().nz;
    let block = r.read_volume().unwrap();
    let DataView::Float32(original) = block.data() else {
        panic!("expected Float32")
    };

    let out = TempMrc::new("setdata");
    {
        let mut w = create(out.path())
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.set_data(original).unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    assert!(r2.validate_header_stats().is_ok());
    let block2 = r2.read_volume().unwrap();
    let DataView::Float32(back) = block2.data() else {
        panic!("expected Float32")
    };
    assert_eq!(original.len(), back.len());
    for (a, b) in original.iter().zip(back.iter()) {
        assert!((a - b).abs() < 1e-6, "data mismatch in set_data roundtrip");
    }
}

#[test]
fn writer_write_block_as_i16() {
    // Read Float32 file, write as Int16 via write_block_as
    let r = assert_open_ok(BOX320);
    let nx = r.shape().nx;
    let ny = r.shape().ny;
    let nz = r.shape().nz;
    let block = r.convert::<f32>().read_volume().unwrap();

    let out = TempMrc::new("wba_i16");
    {
        let mut w = create(out.path())
            .shape([nx, ny, nz])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_block_as(&mrc::VoxelBlock::new([0, 0, 0], [nx, ny, nz], block.data).unwrap())
            .unwrap();
        w.update_header_stats().unwrap();
        w.finalize().unwrap();
    }

    let r2 = Reader::open(out.path()).unwrap();
    assert_eq!(r2.mode(), Mode::Int16);
    // Values are 0 or 1 from binary mask
    let block2 = r2.read_volume().unwrap();
    let DataView::Int16(back) = block2.data() else {
        panic!("expected Int16")
    };
    for &v in back {
        assert!(v == 0 || v == 1, "unexpected value {v} in converted mask");
    }
}
