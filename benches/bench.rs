//! Criterion benchmarks for `mrc` crate I/O and conversion performance.
//!
//! Run with:
//! ```text
//! cargo bench --all-features
//! ```

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use mrc::{VoxelBlock, create};

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
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data).unwrap())
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
        w.write_block(&VoxelBlock::new([0, 0, 0], [nx, ny, nz], data).unwrap())
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
            let vol = reader.read_volume::<f32>().unwrap();
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
            for slice in reader.slices::<f32>() {
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
            let vol = reader.read_volume::<f32>().unwrap();
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
    let block = VoxelBlock::new([0, 0, 0], [NX, NY, NZ], data).unwrap();

    c.bench_function("write_block_full_f32", |b| {
        b.iter(|| {
            let mut w = create(black_box(&p))
                .shape([NX, NY, NZ])
                .mode::<f32>()
                .finish()
                .unwrap();
            w.write_block(black_box(&block)).unwrap();
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
        w.write_block(&VoxelBlock::new([0, 0, 0], [1024, 1024, 1], src).unwrap())
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
);
criterion_main!(benches);
