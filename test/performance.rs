use criterion::{Criterion, black_box, criterion_group, criterion_main};
use mrc::Header;
#[cfg(feature = "std")]
use mrc::MrcFile;
#[cfg(feature = "mmap")]
use mrc::MrcMmap;
use mrc::MrcView;
use tempfile::NamedTempFile;

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

fn bench_header_data_size_calculation(c: &mut Criterion) {
    let mut header = Header::new();
    header.nx = 512;
    header.ny = 512;
    header.nz = 512;
    header.mode = 2;

    c.bench_function("data_size_calculation", |b| {
        b.iter(|| black_box(header.data_size()))
    });
}

fn bench_header_read_write(c: &mut Criterion) {
    let temp_file = NamedTempFile::new().unwrap();
    let mut header = Header::new();
    header.nx = 256;
    header.ny = 256;
    header.nz = 100;
    header.mode = 2;

    // Create test file

    {
        let data = vec![0u8; header.data_size()];

        let mut file = MrcFile::create(temp_file.path(), header.clone()).unwrap();

        file.write_data(&data).unwrap();
    }

    c.bench_function("header_read", |b| {
        b.iter(|| {
            let file = MrcFile::open(temp_file.path()).unwrap();
            let _data = file.read_data().unwrap().to_vec();
            black_box(file.header().clone())
        })
    });
}

fn bench_sequential_read_1gb(c: &mut Criterion) {
    let temp_file = NamedTempFile::new().unwrap();
    let mut header = Header::new();
    header.nx = 1024;
    header.ny = 1024;
    header.nz = 256;
    header.mode = 2;

    // Create 1GB test file
    {
        let data = vec![0u8; header.data_size()];
        let mut file = MrcFile::create(temp_file.path(), header).unwrap();
        file.write_data(&data).unwrap();
    }

    c.bench_function("sequential_read_1gb", |b| {
        b.iter(|| {
            let file = MrcFile::open(temp_file.path()).unwrap();
            let data = file.read_data().unwrap().to_vec();
            black_box(data)
        })
    });
}

fn bench_sequential_read_10gb(c: &mut Criterion) {
    let temp_file = NamedTempFile::new().unwrap();
    let mut header = Header::new();
    header.nx = 2048;
    header.ny = 2048;
    header.nz = 640;
    header.mode = 2;

    // Create 10GB test file
    {
        let data = vec![0u8; header.data_size()];
        let mut file = MrcFile::create(temp_file.path(), header).unwrap();
        file.write_data(&data).unwrap();
    }

    c.bench_function("sequential_read_10gb", |b| {
        b.iter(|| {
            let file = MrcFile::open(temp_file.path()).unwrap();
            let data = file.read_data().unwrap().to_vec();
            black_box(data)
        })
    });
}

#[cfg(feature = "mmap")]
fn bench_mmap_sequential_read_1gb(c: &mut Criterion) {
    let temp_file = NamedTempFile::new().unwrap();
    let mut header = Header::new();
    header.nx = 1024;
    header.ny = 1024;
    header.nz = 256;
    header.mode = 2;

    // Create 1GB test file
    {
        let data = vec![0u8; header.data_size()];
        let mut file = MrcFile::create(temp_file.path(), header).unwrap();
        file.write_data(&data).unwrap();
    }

    c.bench_function("mmap_sequential_read_1gb", |b| {
        b.iter(|| {
            let file = MrcMmap::open(temp_file.path()).unwrap();
            black_box(file.data().to_vec())
        })
    });
}

fn bench_cache_line_alignment(c: &mut Criterion) {
    let temp_file = NamedTempFile::new().unwrap();

    let mut header = Header::new();

    header.nx = 1024;

    header.ny = 1024;

    header.nz = 1;

    header.mode = 2;

    // Create test file

    {
        let data = vec![0u8; header.data_size()];

        let mut file = MrcFile::create(temp_file.path(), header).unwrap();

        file.write_data(&data).unwrap();
    }

    c.bench_function("cache_line_aligned_access", |b| {
        b.iter(|| {
            let file = MrcFile::open(temp_file.path()).unwrap();

            let data = file.read_data().unwrap().to_vec();

            let view = MrcView::from_parts(file.header().clone(), &[], &data).unwrap();

            let typed = black_box(view.data.as_f32().unwrap());

            black_box(typed.len())
        })
    });
}

criterion_group!(
    benches,
    bench_header_creation,
    bench_header_validation,
    bench_header_data_size_calculation,
    bench_header_read_write,
    bench_sequential_read_1gb,
    bench_sequential_read_10gb,
    bench_cache_line_alignment
);

criterion_group!(
    name = mmap_benches;
    config = Criterion::default();
    targets =
        bench_header_creation,
        bench_header_validation,
        bench_header_data_size_calculation,
        bench_header_read_write,
        bench_sequential_read_1gb,
        bench_sequential_read_10gb,
        bench_mmap_sequential_read_1gb,
        bench_cache_line_alignment
);

criterion_main!(benches);
