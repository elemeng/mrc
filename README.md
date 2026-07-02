# mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc)
[![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)

> **Type-safe MRC-2014 file format reader/writer for Rust**

A high-performance, memory-efficient library for reading and writing MRC (Medical Research Council) format files used in cryo-electron microscopy and structural biology.

**Note:** This crate is under active development. While most features are functional, occasional API changes are possible. Contributions welcome — please report issues and share your ideas!

## Installation

```toml
[dependencies]
mrc = "0.2"
```

## Quick Start

```rust
use mrc::{open, create, VoxelBlock};

// Read (auto-detects compression; handles common microscope quirks
// like NVERSION=0 and "MAP\0" automatically)
let reader = open("protein.mrc")?;
for slice in reader.convert_slices::<f32>() {
    let block = slice?;  // Vec<f32>
}

// Write — always call .finalize() or the header will be incomplete
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;
writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [512, 512, 1], vec![0.0f32; 512 * 512],
)?)?;
writer.finalize()?;
```

## Full Documentation

See **[docs.rs/mrc](https://docs.rs/mrc)** for the complete API reference, including:

- Reading files — `Reader`, `MmapReader`, compressed I/O, permissive mode,
  decompression bomb protection (256 GiB limit, configurable)
- Writing files — `Writer`, `MmapWriter`, `GzipWriter`, `Bzip2Writer`
- Iterators — slices, slabs, tiles, volumes,
  generic `convert_slices::<T>()` / `convert_slabs::<T>()`
- Full-volume reads — `read_volume::<T>()`, `read_volume_u8()`,
  generic `convert_volume::<T>()`
- Data modes — `Mode` enum and compile-time `Voxel` trait, including Packed4Bit
  read/write via `slices_u8` / `write_u4_block`
- Headers — `Header`, `HeaderBuilder`, validation, endianness
- FEI extended headers — typed `Fei1Metadata` / `Fei2Metadata` parsing
- Error handling — `Error` and `HeaderValidationError`
- Validation — `validate_full` / `validate_reader` / `ValidationReport`
- **Real-world workflows** — tilt series, FEI metadata, volume stacks
- **Troubleshooting** — common errors and how to fix them

## CLI Tools

| Binary | Description |
|--------|-------------|
| `mrc-validate` | Comprehensive file validation (header, stats, NaN/Inf scan) |
| `mrc-header`   | Human-readable header inspection with semantic interpretation |
| `mrc-invert`   | Contrast inversion (v → −v) with updated header statistics |

```bash
cargo build --release --bin mrc-validate
./mrc-validate protein.mrc
```

## Migrating from v0.1

| v0.1 | v0.2 |
|------|------|
| `MrcView::new(data)` | `Reader::open(path)` / `open(path)` |
| `MrcFile::create(path, header)` | `create(path).shape(dims).mode::<T>().finish()` |
| `MrcView::view::<f32>()` | `reader.slices::<f32>()` |
| `MrcViewMut` | `Writer` + `VoxelBlock<T>` |
| `MrcMmap` | `MmapReader` / `MmapWriter` |

v0.2 adds SIMD acceleration, parallel encoding, type conversion iterators, compression support, unified reader API, and FEI extended header parsing.

## Roadmap

**v0.2.x** — Core + SIMD + FEI (current)

- [x] Complete MRC-2014 format support
- [x] Iterator-centric API (slices, slabs, tiles)
- [x] Type-safe I/O with compile-time mode checking
- [x] SIMD acceleration (AVX2, NEON)
- [x] Memory-mapped I/O and parallel encoding
- [x] All data types (modes 0–4, 6, 12, 101)
- [x] Compression support (gzip, bzip2)
- [x] FEI1/FEI2 extended header parsing
- [x] Header statistics computation and validation
- [x] Permissive mode and volume stack support
- [x] Decompression bomb protection (configurable 256 GiB limit)

**v0.3.x** — Extended Features

- [ ] Extended header parsing for CCP4, MRCO, SERI, AGAR formats
- [ ] Dedicated benchmark suite

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgments

- [CCP-EM](https://www.ccpem.ac.uk/) for the MRC-2014 specification
- EMDB for providing real-world test data
- Cryo-EM community for invaluable feedback
