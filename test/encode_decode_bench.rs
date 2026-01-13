use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use mrc::{DecodeFromFile, EncodeToFile, FileEndian, Mode, DataBlock, DataBlockMut};

// Benchmark decode operations for different types and sizes

fn bench_decode_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_f32");

    for size in [1024, 16384, 262144, 1048576].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i as u8)).collect();

        group.bench_with_input(BenchmarkId::new("little_endian", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len() / 4);
                for chunk in data.chunks_exact(4) {
                    result.push(f32::decode(FileEndian::LittleEndian, chunk));
                }
                black_box(result)
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len() / 4);
                for chunk in data.chunks_exact(4) {
                    result.push(f32::decode(FileEndian::BigEndian, chunk));
                }
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_decode_i16(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_i16");

    for size in [1024, 16384, 262144, 1048576].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i as u8)).collect();

        group.bench_with_input(BenchmarkId::new("little_endian", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len() / 2);
                for chunk in data.chunks_exact(2) {
                    result.push(i16::decode(FileEndian::LittleEndian, chunk));
                }
                black_box(result)
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len() / 2);
                for chunk in data.chunks_exact(2) {
                    result.push(i16::decode(FileEndian::BigEndian, chunk));
                }
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_decode_u16(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_u16");

    for size in [1024, 16384, 262144, 1048576].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i as u8)).collect();

        group.bench_with_input(BenchmarkId::new("little_endian", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len() / 2);
                for chunk in data.chunks_exact(2) {
                    result.push(u16::decode(FileEndian::LittleEndian, chunk));
                }
                black_box(result)
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len() / 2);
                for chunk in data.chunks_exact(2) {
                    result.push(u16::decode(FileEndian::BigEndian, chunk));
                }
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_decode_i8(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_i8");

    for size in [1024, 16384, 262144, 1048576].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i as u8)).collect();

        group.bench_with_input(BenchmarkId::new("native", size), &data, |b, data| {
            b.iter(|| {
                let mut result = Vec::with_capacity(data.len());
                for &byte in data {
                    result.push(i8::decode(FileEndian::LittleEndian, &[byte]));
                }
                black_box(result)
            })
        });
    }

    group.finish();
}

// Benchmark encode operations

fn bench_encode_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_f32");

    for count in [256, 4096, 65536, 262144].iter() {
        let values: Vec<f32> = (0..*count).map(|i| i as f32).collect();
        let mut output = vec![0u8; count * 4];

        group.bench_with_input(BenchmarkId::new("little_endian", count), &values, |b, values| {
            b.iter(|| {
                for (i, &value) in values.iter().enumerate() {
                    value.encode(FileEndian::LittleEndian, &mut output[i * 4..i * 4 + 4]);
                }
                black_box(output.len())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", count), &values, |b, values| {
            b.iter(|| {
                for (i, &value) in values.iter().enumerate() {
                    value.encode(FileEndian::BigEndian, &mut output[i * 4..i * 4 + 4]);
                }
                black_box(output.len())
            })
        });
    }

    group.finish();
}

fn bench_encode_i16(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_i16");

    for count in [256, 4096, 65536, 262144].iter() {
        let values: Vec<i16> = (0..*count).map(|i| i as i16).collect();
        let mut output = vec![0u8; count * 2];

        group.bench_with_input(BenchmarkId::new("little_endian", count), &values, |b, values| {
            b.iter(|| {
                for (i, &value) in values.iter().enumerate() {
                    value.encode(FileEndian::LittleEndian, &mut output[i * 2..i * 2 + 2]);
                }
                black_box(output.len())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", count), &values, |b, values| {
            b.iter(|| {
                for (i, &value) in values.iter().enumerate() {
                    value.encode(FileEndian::BigEndian, &mut output[i * 2..i * 2 + 2]);
                }
                black_box(output.len())
            })
        });
    }

    group.finish();
}

fn bench_encode_u16(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_u16");

    for count in [256, 4096, 65536, 262144].iter() {
        let values: Vec<u16> = (0..*count).map(|i| i as u16).collect();
        let mut output = vec![0u8; count * 2];

        group.bench_with_input(BenchmarkId::new("little_endian", count), &values, |b, values| {
            b.iter(|| {
                for (i, &value) in values.iter().enumerate() {
                    value.encode(FileEndian::LittleEndian, &mut output[i * 2..i * 2 + 2]);
                }
                black_box(output.len())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", count), &values, |b, values| {
            b.iter(|| {
                for (i, &value) in values.iter().enumerate() {
                    value.encode(FileEndian::BigEndian, &mut output[i * 2..i * 2 + 2]);
                }
                black_box(output.len())
            })
        });
    }

    group.finish();
}

// Benchmark DataBlock as_* methods (high-level API)

fn bench_datablock_as_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("datablock_as_f32");

    for voxel_count in [256, 4096, 65536, 262144].iter() {
        let data: Vec<u8> = vec![0u8; voxel_count * 4];

        group.bench_with_input(BenchmarkId::new("little_endian", voxel_count), &data, |b, data| {
            b.iter(|| {
                let block = DataBlock::new(data, Mode::Float32, FileEndian::LittleEndian);
                black_box(block.as_f32().unwrap())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", voxel_count), &data, |b, data| {
            b.iter(|| {
                let block = DataBlock::new(data, Mode::Float32, FileEndian::BigEndian);
                black_box(block.as_f32().unwrap())
            })
        });
    }

    group.finish();
}

fn bench_datablock_as_i16(c: &mut Criterion) {
    let mut group = c.benchmark_group("datablock_as_i16");

    for voxel_count in [256, 4096, 65536, 262144].iter() {
        let data: Vec<u8> = vec![0u8; voxel_count * 2];

        group.bench_with_input(BenchmarkId::new("little_endian", voxel_count), &data, |b, data| {
            b.iter(|| {
                let block = DataBlock::new(data, Mode::Int16, FileEndian::LittleEndian);
                black_box(block.as_i16().unwrap())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", voxel_count), &data, |b, data| {
            b.iter(|| {
                let block = DataBlock::new(data, Mode::Int16, FileEndian::BigEndian);
                black_box(block.as_i16().unwrap())
            })
        });
    }

    group.finish();
}

// Benchmark DataBlockMut set_* methods (high-level API)

fn bench_datablock_set_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("datablock_set_f32");

    for voxel_count in [256, 4096, 65536, 262144].iter() {
        let values: Vec<f32> = (0..*voxel_count).map(|i| i as f32).collect();
        let mut data = vec![0u8; voxel_count * 4];

        group.bench_with_input(BenchmarkId::new("little_endian", voxel_count), &values, |b, values| {
            b.iter(|| {
                let mut block = DataBlockMut::new(&mut data, Mode::Float32, FileEndian::LittleEndian);
                black_box(block.set_f32(values).unwrap())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", voxel_count), &values, |b, values| {
            b.iter(|| {
                let mut block = DataBlockMut::new(&mut data, Mode::Float32, FileEndian::BigEndian);
                black_box(block.set_f32(values).unwrap())
            })
        });
    }

    group.finish();
}

fn bench_datablock_set_i16(c: &mut Criterion) {
    let mut group = c.benchmark_group("datablock_set_i16");

    for voxel_count in [256, 4096, 65536, 262144].iter() {
        let values: Vec<i16> = (0..*voxel_count).map(|i| i as i16).collect();
        let mut data = vec![0u8; voxel_count * 2];

        group.bench_with_input(BenchmarkId::new("little_endian", voxel_count), &values, |b, values| {
            b.iter(|| {
                let mut block = DataBlockMut::new(&mut data, Mode::Int16, FileEndian::LittleEndian);
                black_box(block.set_i16(values).unwrap())
            })
        });

        group.bench_with_input(BenchmarkId::new("big_endian", voxel_count), &values, |b, values| {
            b.iter(|| {
                let mut block = DataBlockMut::new(&mut data, Mode::Int16, FileEndian::BigEndian);
                black_box(block.set_i16(values).unwrap())
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_decode_f32,
    bench_decode_i16,
    bench_decode_u16,
    bench_decode_i8,
    bench_encode_f32,
    bench_encode_i16,
    bench_encode_u16,
    bench_datablock_as_f32,
    bench_datablock_as_i16,
    bench_datablock_set_f32,
    bench_datablock_set_i16
);

criterion_main!(benches);