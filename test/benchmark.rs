use criterion::{Criterion, black_box, criterion_group, criterion_main};
use mrc::{Header, MrcView};

fn bench_header_creation(c: &mut Criterion) {
    c.bench_function("header_creation", |b| b.iter(|| Header::new()));
}

fn bench_header_validation(c: &mut Criterion) {
    let mut header = Header::new();
    header.nx = 100;
    header.ny = 100;
    header.nz = 100;
    header.mode = 2;

    c.bench_function("header_validation", |b| {
        b.iter(|| black_box(header.validate()))
    });
}

fn bench_data_size_calculation(c: &mut Criterion) {
    let mut header = Header::new();
    header.nx = 512;
    header.ny = 512;
    header.nz = 512;
    header.mode = 2;

    c.bench_function("data_size_calculation", |b| {
        b.iter(|| black_box(header.data_size()))
    });
}

fn bench_map_creation(c: &mut Criterion) {
    let mut header = Header::new();
    header.nx = 64;
    header.ny = 64;
    header.nz = 64;
    header.mode = 2;

    let data = vec![0u8; 64 * 64 * 64 * 4];

    c.bench_function("map_creation", |b| {
        b.iter(|| MrcView::new(header, &data).unwrap())
    });
}

fn bench_view_access(c: &mut Criterion) {
    let mut header = Header::new();
    header.nx = 64;
    header.ny = 64;
    header.nz = 64;
    header.mode = 2;

    let data = vec![0u8; 64 * 64 * 64 * 4];
    let map = MrcView::new(header, &data).unwrap();

    c.bench_function("view_access_f32", |b| {
        b.iter(|| {
            let view: &[f32] = map.view().unwrap();
            black_box(view.len())
        })
    });
}

fn bench_slice_access(c: &mut Criterion) {
    let mut header = Header::new();
    header.nx = 64;
    header.ny = 64;
    header.nz = 64;
    header.mode = 2;

    let data = vec![0u8; 64 * 64 * 64 * 4];
    let map = MrcView::new(header, &data).unwrap();

    c.bench_function("slice_bytes_access", |b| {
        b.iter(|| {
            let slice = map.slice_bytes(0..1024).unwrap();
            black_box(slice.len())
        })
    });
}

criterion_group!(
    benches,
    bench_header_creation,
    bench_header_validation,
    bench_data_size_calculation,
    bench_map_creation,
    bench_view_access,
    bench_slice_access
);
criterion_main!(benches);
