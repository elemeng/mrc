//! Criterion benchmarks for `mrc` crate I/O and conversion performance.
//!
//! Run with:
//! ```text
//! cargo bench --all-features
//! ```

use criterion::{Criterion, criterion_group, criterion_main};
use mrc::{DataBlock, OwnedData, create};
use std::hint::black_box;
use std::path::Path;

// ============================================================================
// Helpers — generate temp MRC files for benchmarking
// ============================================================================

/// Create a temporary Float32 MRC file and return its path.
fn make_f32_mrc(nx: usize, ny: usize, nz: usize) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("mrc_bench_f32_{nx}x{ny}x{nz}.mrc"));
    let _ = std::fs::remove_file(&p);

    let total = nx * ny * nz;
    let data: Vec<f32> = (0..total).map(|i| (i % 1000) as f32).collect();
    {
        let mut w = create(&p)
            .shape([nx, ny, nz])
            .mode::<f32>()
            .finish()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [nx, ny, nz],
            data: OwnedData::Float32(data),
        })
        .unwrap();
        w.finalize().unwrap();
    }
    p
}

/// Create a temporary Int16 MRC file (for conversion benchmarks).
fn make_i16_mrc(nx: usize, ny: usize, nz: usize) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("mrc_bench_i16_{nx}x{ny}x{nz}.mrc"));
    let _ = std::fs::remove_file(&p);

    let total = nx * ny * nz;
    let data: Vec<i16> = (0..total).map(|i| (i % 1000 - 500) as i16).collect();
    {
        let mut w = create(&p)
            .shape([nx, ny, nz])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [nx, ny, nz],
            data: OwnedData::Int16(data),
        })
        .unwrap();
        w.finalize().unwrap();
    }
    p
}

/// Benchmark parameters: a modest 256×256×256 volume.
const NX: usize = 256;
const NY: usize = 256;
const NZ: usize = 64;

// ============================================================================
// Benchmarks: read_volume (buffered Reader)
// ============================================================================

fn bench_read_volume_f32(c: &mut Criterion) {
    let path = make_f32_mrc(NX, NY, NZ);
    c.bench_function("read_volume_f32", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&path)).unwrap();
            let vol = reader.read_volume().unwrap();
            black_box(vol);
        })
    });
    let _ = std::fs::remove_file(&path);
}

fn bench_read_volume_via_convert_f32(c: &mut Criterion) {
    let path = make_i16_mrc(NX, NY, NZ);
    c.bench_function("convert_i16_to_f32_volume", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&path)).unwrap();
            let vol = reader.convert::<f32>().read_volume().unwrap();
            black_box(vol);
        })
    });
    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// Benchmarks: slice iteration
// ============================================================================

fn bench_iterate_slices_f32(c: &mut Criterion) {
    let path = make_f32_mrc(NX, NY, NZ);
    c.bench_function("iterate_slices_f32", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&path)).unwrap();
            for slice in reader.slices() {
                let _ = black_box(slice.unwrap());
            }
        })
    });
    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// Benchmarks: mmap reader
// ============================================================================

#[cfg(feature = "mmap")]
fn bench_mmap_read_volume_f32(c: &mut Criterion) {
    let path = make_f32_mrc(NX, NY, NZ);
    c.bench_function("mmap_read_volume_f32", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&path)).unwrap();
            let vol = reader.read_volume().unwrap();
            black_box(vol);
        })
    });
    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// Benchmarks: write throughput
// ============================================================================

fn bench_write_block_full_volume(c: &mut Criterion) {
    let mut p = std::env::temp_dir();
    p.push("mrc_bench_write.mrc");
    let _ = std::fs::remove_file(&p);

    let total = NX * NY * NZ;
    let data: Vec<f32> = (0..total).map(|i| (i % 1000) as f32).collect();
    let block = DataBlock::Owned {
        offset: [0, 0, 0],
        shape: [NX, NY, NZ],
        data: OwnedData::Float32(data),
    };

    c.bench_function("write_block_full_f32", |b| {
        b.iter(|| {
            let mut w = create(black_box(&p))
                .shape([NX, NY, NZ])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_data_block(black_box(&block)).unwrap();
            w.finalize().unwrap();
        })
    });
    let _ = std::fs::remove_file(&p);
}

// ============================================================================
// Benchmarks: stats computation
// ============================================================================

fn bench_compute_stats_f32(c: &mut Criterion) {
    let path = make_f32_mrc(NX, NY, NZ);
    c.bench_function("compute_stats_f32", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&path)).unwrap();
            let result = reader.validate_header_stats();
            let _ = black_box(result);
        })
    });
    let _ = std::fs::remove_file(&path);
}

fn bench_compute_stats_i16(c: &mut Criterion) {
    let path = make_i16_mrc(NX, NY, NZ);
    c.bench_function("compute_stats_i16", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&path)).unwrap();
            let result = reader.validate_header_stats();
            let _ = black_box(result);
        })
    });
    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// Benchmarks: SIMD vs scalar conversion (i16 → f32 via public API)
// ============================================================================

fn bench_convert_i16_to_f32_slice(c: &mut Criterion) {
    let src: Vec<i16> = (0..1024 * 1024).map(|i| (i % 1000 - 500) as i16).collect();

    // Use the reader's convert path to exercise SIMD/scalar conversion.
    // Write a small MRC file, then read and convert it.
    let mut p = std::env::temp_dir();
    p.push("mrc_bench_convert.mrc");
    let _ = std::fs::remove_file(&p);
    {
        let mut w = create(&p)
            .shape([1024, 1024, 1])
            .mode::<i16>()
            .finish()
            .unwrap();
        w.write_data_block(&DataBlock::Owned {
            offset: [0, 0, 0],
            shape: [1024, 1024, 1],
            data: OwnedData::Int16(src),
        })
        .unwrap();
        w.finalize().unwrap();
    }

    c.bench_function("convert_i16_to_f32_1M_via_reader", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(&p)).unwrap();
            let vol = reader.convert::<f32>().read_volume().unwrap();
            black_box(vol);
        })
    });
    let _ = std::fs::remove_file(&p);
}

// ============================================================================
// Benchmarks: real data files from real_data/
// ============================================================================

const REAL_SEARCH: &str = "real_data/search.mrc";
const REAL_BOX320: &str = "real_data/box320_bin2_mask.mrc";
const REAL_EMMAP: &str = "real_data/em-map.mrc";
const REAL_TOMOGRAM: &str = "real_data/tomogram.mrc";

/// Check all required real data files exist, panicking once with a summary.
/// Called once per process from any real-data benchmark function.
fn ensure_real_data_available() {
    use std::sync::Once;
    static CHECK: Once = Once::new();
    CHECK.call_once(|| {
        let missing: Vec<&str> = [REAL_SEARCH, REAL_BOX320, REAL_EMMAP, REAL_TOMOGRAM]
            .into_iter()
            .filter(|p| !Path::new(p).exists())
            .collect();
        if !missing.is_empty() {
            panic!(
                "real data files not found ({} missing):\n  {}\n\
                 The real_data/ directory is gitignored (files are too large for GitHub).\n\
                 Place any MRC files here (from EMDB, EMPIAR, or your own microscope)\n\
                 and the benchmarks will exercise every API path against them.",
                missing.len(),
                missing.join("\n  "),
            );
        }
    });
}

fn bench_real_read_volume_search(c: &mut Criterion) {
    ensure_real_data_available();
    c.bench_function("real/read_volume/search", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_SEARCH)).unwrap();
            let vol = reader.read_volume().unwrap();
            black_box(vol);
        })
    });
}

fn bench_real_read_volume_box320(c: &mut Criterion) {
    c.bench_function("real/read_volume/box320", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            let vol = reader.read_volume().unwrap();
            black_box(vol);
        })
    });
}

fn bench_real_subregion_search(c: &mut Criterion) {
    c.bench_function("real/subregion/search_256x256x4", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_SEARCH)).unwrap();
            let block = reader.subregion([100, 100, 2], [256, 256, 4]).unwrap();
            black_box(block);
        })
    });
}

fn bench_real_subregion_emmap(c: &mut Criterion) {
    c.bench_function("real/subregion/emmap_128x128x128", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_EMMAP)).unwrap();
            let block = reader.subregion([32, 32, 32], [128, 128, 128]).unwrap();
            black_box(block);
        })
    });
}

fn bench_real_iterate_slices_box320(c: &mut Criterion) {
    c.bench_function("real/iterate_slices/box320_10slices", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            for slice in reader.convert::<f32>().slices().take(10) {
                let _ = black_box(slice.unwrap());
            }
        })
    });
}

fn bench_real_iterate_slabs_box320(c: &mut Criterion) {
    c.bench_function("real/iterate_slabs/box320_32px", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            for slab in reader.convert::<f32>().slabs(32).take(4) {
                let _ = black_box(slab.unwrap());
            }
        })
    });
}

fn bench_real_compute_stats_box320(c: &mut Criterion) {
    c.bench_function("real/compute_stats/box320", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            let result = reader.validate_header_stats();
            let _ = black_box(result);
        })
    });
}

fn bench_real_compute_stats_search(c: &mut Criterion) {
    c.bench_function("real/compute_stats/search", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_SEARCH)).unwrap();
            let result = reader.validate_header_stats();
            let _ = black_box(result);
        })
    });
}

fn bench_real_convert_i16_to_f32_search(c: &mut Criterion) {
    c.bench_function("real/convert_search_i16_to_f32", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_SEARCH)).unwrap();
            let vol = reader.convert::<f32>().read_volume().unwrap();
            black_box(vol);
        })
    });
}

fn bench_real_read_slices_tomogram(c: &mut Criterion) {
    c.bench_function("real/read_slices/tomogram_5slices", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for slice in reader.convert::<f32>().slices().take(5) {
                let _ = black_box(slice.unwrap());
            }
        })
    });
}

#[cfg(feature = "mmap")]
fn bench_real_mmap_read_volume_box320(c: &mut Criterion) {
    c.bench_function("real/mmap_read_volume/box320", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            let vol = reader.read_volume().unwrap();
            black_box(vol);
        })
    });
}

#[cfg(feature = "mmap")]
fn bench_real_mmap_subregion_tomogram(c: &mut Criterion) {
    c.bench_function("real/mmap_subregion/tomogram_512x512x128", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader.subregion([128, 256, 50], [512, 512, 128]).unwrap();
            black_box(block);
        })
    });
}

fn bench_real_iterate_tiles_search(c: &mut Criterion) {
    c.bench_function("real/tiles/search_512x512x2", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_SEARCH)).unwrap();
            for tile in reader.tiles([512, 512, 2]).unwrap().take(5) {
                let _ = black_box(tile.unwrap());
            }
        })
    });
}

fn bench_real_iterate_tiles_box320(c: &mut Criterion) {
    c.bench_function("real/tiles/box320_160x160x160", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            for tile in reader.tiles([160, 160, 160]).unwrap().take(3) {
                let _ = black_box(tile.unwrap());
            }
        })
    });
}

fn bench_real_convert_slabs_f32_box320(c: &mut Criterion) {
    c.bench_function("real/convert_slabs/box320_k64_3slabs", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_BOX320)).unwrap();
            for slab in reader.convert::<f32>().slabs(64).take(3) {
                let _ = black_box(slab.unwrap());
            }
        })
    });
}

fn bench_real_read_as_f32(c: &mut Criterion) {
    c.bench_function("real/read_as/search", |b| {
        b.iter(|| {
            let (header, data): (_, Vec<f32>) = mrc::read_as(black_box(REAL_SEARCH)).unwrap();
            black_box((header, data));
        })
    });
}

fn bench_real_open_permissive(c: &mut Criterion) {
    c.bench_function("real/open_permissive/search", |b| {
        b.iter(|| {
            let (reader, _warnings) = mrc::Reader::open_permissive(black_box(REAL_SEARCH)).unwrap();
            black_box(reader);
        })
    });
}

fn bench_real_validate_full(c: &mut Criterion) {
    c.bench_function("real/validate_full/emmap", |b| {
        b.iter(|| {
            let report = mrc::validate_full(black_box(REAL_EMMAP), true).unwrap();
            black_box(report);
        })
    });
}

// ============================================================================
// Tomogram benchmarks — 1022×1440×750 Float32 (4.2 GB)
// Covers every applicable data path to identify bottlenecks.
// ============================================================================

/// Constants for tomogram dimensions (reused across bench functions).
const TOMO_NX: usize = 1022;
const TOMO_NY: usize = 1440;

// ── Level 0: minimal overhead — file open ─────────────────────────────

fn bench_tomo_open(c: &mut Criterion) {
    ensure_real_data_available();
    c.bench_function("tomogram/open", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            black_box(reader);
        })
    });
}

// ── Level 1: zero-copy metadata access ────────────────────────────────

fn bench_tomo_raw_bytes(c: &mut Criterion) {
    c.bench_function("tomogram/raw_bytes", |b| {
        let reader = mrc::Reader::open(REAL_TOMOGRAM).unwrap();
        b.iter(|| {
            let bytes = reader.raw_bytes();
            black_box(bytes.len());
        })
    });
}

// ── Level 2: single-slice / small-block reads ─────────────────────────

fn bench_tomo_slice_at_mid(c: &mut Criterion) {
    c.bench_function("tomogram/slice/mid_z375", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader
                .subregion([0, 0, 375], [TOMO_NX, TOMO_NY, 1])
                .unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_slice_first(c: &mut Criterion) {
    c.bench_function("tomogram/slice/first_z0", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader.subregion([0, 0, 0], [TOMO_NX, TOMO_NY, 1]).unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_read_block_bytes_contiguous(c: &mut Criterion) {
    c.bench_function("tomogram/block_bytes/contiguous_256z", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let bytes = reader
                .read_block_bytes([0, 0, 0], [TOMO_NX, TOMO_NY, 256])
                .unwrap();
            black_box(bytes.len());
        })
    });
}

fn bench_tomo_read_block_bytes_scatter(c: &mut Criterion) {
    c.bench_function("tomogram/block_bytes/scatter_512x512x64", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let bytes = reader
                .read_block_bytes([256, 384, 50], [512, 512, 64])
                .unwrap();
            black_box(bytes.len());
        })
    });
}

// ── Level 3: subregion reads of various sizes and alignments ──────────

fn bench_tomo_subregion_aligned_128z(c: &mut Criterion) {
    c.bench_function("tomogram/subregion/aligned_xy_128z", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader
                .subregion([0, 0, 100], [TOMO_NX, TOMO_NY, 128])
                .unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_subregion_256x256x128(c: &mut Criterion) {
    c.bench_function("tomogram/subregion/256x256x128", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader.subregion([128, 256, 50], [256, 256, 128]).unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_subregion_512x512x64(c: &mut Criterion) {
    c.bench_function("tomogram/subregion/512x512x64", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader.subregion([200, 400, 20], [512, 512, 64]).unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_subregion_64x64x16(c: &mut Criterion) {
    c.bench_function("tomogram/subregion/64x64x16", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader.subregion([500, 600, 100], [64, 64, 16]).unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_subregion_full_xy_3z(c: &mut Criterion) {
    c.bench_function("tomogram/subregion/full_xy_3zslices", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader
                .subregion([0, 0, 200], [TOMO_NX, TOMO_NY, 3])
                .unwrap();
            black_box(block);
        })
    });
}

// ── Level 4: sequential slice iteration ───────────────────────────────

fn bench_tomo_iterate_all_slices(c: &mut Criterion) {
    c.bench_function("tomogram/iterate/all_slices_750", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for s in reader.slices() {
                let _ = black_box(s.unwrap());
            }
        })
    });
}

fn bench_tomo_iterate_50_slices(c: &mut Criterion) {
    c.bench_function("tomogram/iterate/slices_50", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for s in reader.slices().take(50) {
                let _ = black_box(s.unwrap());
            }
        })
    });
}

// ── Level 5: slab iteration at various depths ─────────────────────────

fn bench_tomo_slabs_k4(c: &mut Criterion) {
    c.bench_function("tomogram/slabs/k4_10slabs", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for slab in reader.slabs(4).take(10) {
                let _ = black_box(slab.unwrap());
            }
        })
    });
}

fn bench_tomo_slabs_k64(c: &mut Criterion) {
    c.bench_function("tomogram/slabs/k64_5slabs", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for slab in reader.slabs(64).take(5) {
                let _ = black_box(slab.unwrap());
            }
        })
    });
}

fn bench_tomo_slabs_k256(c: &mut Criterion) {
    c.bench_function("tomogram/slabs/k256_2slabs", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for slab in reader.slabs(256).take(2) {
                let _ = black_box(slab.unwrap());
            }
        })
    });
}

// ── Level 6: tile iteration ───────────────────────────────────────────

fn bench_tomo_tiles_256x256x64(c: &mut Criterion) {
    c.bench_function("tomogram/tiles/256x256x64_4tiles", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for tile in reader.tiles([256, 256, 64]).unwrap().take(4) {
                let _ = black_box(tile.unwrap());
            }
        })
    });
}

fn bench_tomo_tiles_128x128x32(c: &mut Criterion) {
    c.bench_function("tomogram/tiles/128x128x32_8tiles", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for tile in reader.tiles([128, 128, 32]).unwrap().take(8) {
                let _ = black_box(tile.unwrap());
            }
        })
    });
}

// ── Level 7: ConvertReader paths (all f32 → f32, test overhead) ───────

fn bench_tomo_convert_f32_slices_50(c: &mut Criterion) {
    c.bench_function("tomogram/convert_f32/slices_50", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for s in reader.convert::<f32>().slices().take(50) {
                let _ = black_box(s.unwrap());
            }
        })
    });
}

fn bench_tomo_convert_f32_slabs_k64_5(c: &mut Criterion) {
    c.bench_function("tomogram/convert_f32/slabs_k64_5", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            for slab in reader.convert::<f32>().slabs(64).take(5) {
                let _ = black_box(slab.unwrap());
            }
        })
    });
}

fn bench_tomo_convert_f32_subregion_256x256x128(c: &mut Criterion) {
    c.bench_function("tomogram/convert_f32/subregion_256x256x128", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader
                .convert::<f32>()
                .subregion([128, 256, 50], [256, 256, 128])
                .unwrap();
            black_box(block);
        })
    });
}

fn bench_tomo_convert_f32_subregion_aligned(c: &mut Criterion) {
    c.bench_function("tomogram/convert_f32/subregion_aligned_xy_128z", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let block = reader
                .convert::<f32>()
                .subregion([0, 0, 100], [TOMO_NX, TOMO_NY, 128])
                .unwrap();
            black_box(block);
        })
    });
}

// ── Level 8: stats validation ─────────────────────────────────────────

fn bench_tomo_validate_header_stats(c: &mut Criterion) {
    c.bench_function("tomogram/validate_header_stats", |b| {
        b.iter(|| {
            let reader = mrc::Reader::open(black_box(REAL_TOMOGRAM)).unwrap();
            let result = reader.validate_header_stats();
            let _ = black_box(result);
        })
    });
}

// ============================================================================
// Writer benchmarks using real data — read real file, measure write throughput
// ============================================================================

fn bench_writer_write_volume_search(c: &mut Criterion) {
    // Read search.mrc once, then benchmark writing it
    let reader = mrc::Reader::open(REAL_SEARCH).unwrap();
    let block = reader.convert::<f32>().read_volume().unwrap();
    let nx = block.shape[0];
    let ny = block.shape[1];
    let nz = block.shape[2];
    let data = block.data;

    let mut p = std::env::temp_dir();
    p.push("mrc_bench_write_search.mrc");
    let _ = std::fs::remove_file(&p);

    c.bench_function("writer/write_volume_search_f32", |b| {
        b.iter(|| {
            let mut w = mrc::create(black_box(&p))
                .shape([nx, ny, nz])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_data_block(&DataBlock::Owned {
                offset: [0, 0, 0],
                shape: [nx, ny, nz],
                data: OwnedData::Float32(data.clone()),
            })
            .unwrap();
            w.finalize().unwrap();
        })
    });
    let _ = std::fs::remove_file(&p);
}

fn bench_writer_streaming_slices_search(c: &mut Criterion) {
    // Read search.mrc slices, then benchmark writing them one by one
    let reader = mrc::Reader::open(REAL_SEARCH).unwrap();
    let nx = reader.shape().nx;
    let ny = reader.shape().ny;
    let nz = reader.shape().nz;
    let slices: Vec<Vec<f32>> = reader
        .convert::<f32>()
        .slices()
        .map(|s| s.unwrap().data)
        .collect();

    let mut p = std::env::temp_dir();
    p.push("mrc_bench_write_stream.mrc");
    let _ = std::fs::remove_file(&p);

    c.bench_function("writer/streaming_slices_search_f32", |b| {
        b.iter(|| {
            let mut w = mrc::create(black_box(&p))
                .shape([nx, ny, nz])
                .mode::<f32>()
                .finish()
                .unwrap();
            for (z, slice) in slices.iter().enumerate() {
                w.write_data_block(&DataBlock::Owned {
                    offset: [0, 0, z],
                    shape: [nx, ny, 1],
                    data: OwnedData::Float32(slice.clone()),
                })
                .unwrap();
            }
            w.update_header_stats().unwrap();
            w.finalize().unwrap();
        })
    });
    let _ = std::fs::remove_file(&p);
}

fn bench_writer_write_block_as_search(c: &mut Criterion) {
    // Read search.mrc as f32, write as i16 via write_block_as
    let reader = mrc::Reader::open(REAL_SEARCH).unwrap();
    let block = reader.convert::<f32>().read_volume().unwrap();

    let mut p = std::env::temp_dir();
    p.push("mrc_bench_write_block_as.mrc");
    let _ = std::fs::remove_file(&p);

    c.bench_function("writer/write_block_as_search_f32_to_i16", |b| {
        b.iter(|| {
            let mut w = mrc::create(black_box(&p))
                .shape([block.shape[0], block.shape[1], block.shape[2]])
                .mode::<i16>()
                .finish()
                .unwrap();
            w.write_block_as(
                &mrc::VoxelBlock::new([0, 0, 0], block.shape, block.data.clone()).unwrap(),
            )
            .unwrap();
            w.finalize().unwrap();
        })
    });
    let _ = std::fs::remove_file(&p);
}

// ============================================================================
// Criterion group & main
// ============================================================================

#[cfg(feature = "mmap")]
criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(50);
    targets =
        bench_read_volume_f32,
        bench_read_volume_via_convert_f32,
        bench_iterate_slices_f32,
        bench_mmap_read_volume_f32,
        bench_write_block_full_volume,
        bench_compute_stats_f32,
        bench_compute_stats_i16,
        bench_convert_i16_to_f32_slice,
        bench_real_read_volume_search,
        bench_real_read_volume_box320,
        bench_real_subregion_search,
        bench_real_subregion_emmap,
        bench_real_iterate_slices_box320,
        bench_real_iterate_slabs_box320,
        bench_real_compute_stats_box320,
        bench_real_compute_stats_search,
        bench_real_convert_i16_to_f32_search,
        bench_real_read_slices_tomogram,
        bench_real_mmap_read_volume_box320,
        bench_real_mmap_subregion_tomogram,
        bench_real_iterate_tiles_search,
        bench_real_iterate_tiles_box320,
        bench_real_convert_slabs_f32_box320,
        bench_real_read_as_f32,
        bench_real_open_permissive,
        bench_real_validate_full,
        bench_writer_write_volume_search,
        bench_writer_streaming_slices_search,
        bench_writer_write_block_as_search,
);

#[cfg(not(feature = "mmap"))]
criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(50);
    targets =
        bench_read_volume_f32,
        bench_read_volume_via_convert_f32,
        bench_iterate_slices_f32,
        bench_write_block_full_volume,
        bench_compute_stats_f32,
        bench_compute_stats_i16,
        bench_convert_i16_to_f32_slice,
        bench_real_read_volume_search,
        bench_real_read_volume_box320,
        bench_real_subregion_search,
        bench_real_subregion_emmap,
        bench_real_iterate_slices_box320,
        bench_real_iterate_slabs_box320,
        bench_real_compute_stats_box320,
        bench_real_compute_stats_search,
        bench_real_convert_i16_to_f32_search,
        bench_real_read_slices_tomogram,
        bench_real_iterate_tiles_search,
        bench_real_iterate_tiles_box320,
        bench_real_convert_slabs_f32_box320,
        bench_real_read_as_f32,
        bench_real_open_permissive,
        bench_real_validate_full,
        bench_writer_write_volume_search,
        bench_writer_streaming_slices_search,
        bench_writer_write_block_as_search,
);

criterion_group!(
    name = tomo_benches;
    config = Criterion::default().sample_size(10).measurement_time(std::time::Duration::from_secs(30));
    targets =
        bench_tomo_open,
        bench_tomo_raw_bytes,
        bench_tomo_slice_at_mid,
        bench_tomo_slice_first,
        bench_tomo_read_block_bytes_contiguous,
        bench_tomo_read_block_bytes_scatter,
        bench_tomo_subregion_aligned_128z,
        bench_tomo_subregion_256x256x128,
        bench_tomo_subregion_512x512x64,
        bench_tomo_subregion_64x64x16,
        bench_tomo_subregion_full_xy_3z,
        bench_tomo_iterate_all_slices,
        bench_tomo_iterate_50_slices,
        bench_tomo_slabs_k4,
        bench_tomo_slabs_k64,
        bench_tomo_slabs_k256,
        bench_tomo_tiles_256x256x64,
        bench_tomo_tiles_128x128x32,
        bench_tomo_convert_f32_slices_50,
        bench_tomo_convert_f32_slabs_k64_5,
        bench_tomo_convert_f32_subregion_256x256x128,
        bench_tomo_convert_f32_subregion_aligned,
        bench_tomo_validate_header_stats,
);

criterion_main!(benches, tomo_benches);
