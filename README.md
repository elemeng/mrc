# mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc)
[![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)
[![CI](https://github.com/elemeng/mrc/workflows/CI/badge.svg)](https://github.com/elemeng/mrc/actions)

> **Type-safe MRC-2014 reader/writer for Rust** — SIMD-accelerated, mmap-enabled, with full cryo-EM metadata support.

Read, write, and inspect MRC files — the standard format in cryo-electron microscopy and structural biology. Handles everything from raw tilt series to reconstructed volumes, with automatic endianness detection, compression, and extended header parsing for all major microscope formats.

---

## Quick Start

```rust,no_run
use mrc::{open, create, VoxelBlock};

// Read — auto-detects gzip/bzip2, handles NVERSION=0, "MAP\0" automatically
let reader = open("density.mrc")?;
for slice in reader.convert::<f32>().slices() {
    let block = slice?;
}

// Write — type-safe, compile-time mode checking
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;
writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [512, 512, 1], vec![0.0f32; 512 * 512],
)?)?;
writer.update_header_stats()?;  // compute dmin/dmax/dmean/rms
writer.finalize()?;             // **required** — rewrites header
```

---

## Why `mrc`?

| Feature | What it means for you |
|---|---|
| **Type-safe I/O** | `reader.slices::<f32>()` — wrong type won't compile, not just a runtime error |
| **Zero-copy mmap** | Files too large for RAM? Memory-mapped access with OS demand-paging |
| **Auto-conversion** | `reader.convert::<f32>().slices()` — read any mode as `f32` without thinking |
| **SIMD acceleration** | AVX2/NEON for i8↔f32, i16↔f32, f16↔f32, byte-swap — up to 8x faster |
| **All data modes** | Int8, Int16, Float32, Uint16, Float16, complex, Packed4Bit |
| **Extended headers** | FEI1/FEI2, CCP4, MRCO, SerialEM, Agard — parse metadata from every major microscope |
| **Compression** | gzip/bzip2 auto-detection; decompression bomb protection (256 GiB limit) |
| **Permissive mode** | Open quirky files (bad NVERSION, wrong MACHST) without errors |
| **Volume stacks** | Read multi-sub-volume files with `reader.volumes::<f32>()` |
| **Validation** | `mrc-validate` CLI — header checks, stats cross-check, NaN/Inf scan |
| **No trait imports** | All methods are inherent — `reader.slices()`, not `use SomeTrait` |
| **393 tests** | 212 doc-tests, 60 API coverage tests, 23 integration tests |

---

## Installation

```toml
[dependencies]
mrc = "0.5"
```

Enable optional features in `Cargo.toml`:

```toml
mrc = { version = "0.5", features = ["ndarray", "serde", "bzip2"] }
```

| Feature | Default | What it adds |
|---|---|---|
| `mmap` | ✅ | Memory-mapped I/O (auto-selected for large files) |
| `f16` | ✅ | Half-precision float (`half::f16`) support |
| `simd` | ✅ | AVX2/NEON acceleration |
| `parallel` | ✅ | Parallel encoding via `rayon` |
| `gzip` | ✅ | Gzip auto-detection and compressed writer |
| `bzip2` | ❌ | Bzip2 auto-detection and compressed writer |
| `ndarray` | ❌ | Return volumes as `ndarray::Array3<T>` |
| `serde` | ❌ | Serialize/Deserialize for all public types |

---

## Quick Tour

### Reading

```rust,no_run
use mrc::Reader;

// Open — auto-detects compression and byte order
let reader = Reader::open("tiltseries.mrc")?;
println!("{}×{}×{} voxels, mode {:?}",
    reader.shape().nx, reader.shape().ny, reader.shape().nz,
    reader.mode());

// Iterate — slices, slabs, tiles, or subregion
for slice in reader.slices::<f32>() {          // one Z-plane
    let block = slice?;
}
for slab in reader.slabs::<f32>(16) {          // 16 planes at a time
    let block = slab?;
}
for tile in reader.tiles::<f32>([64, 64, 64])? { // 3D tiles
    let block = tile?;
}

// Full volume in one call
let volume = reader.read_volume::<f32>()?;
println!("{} voxels", volume.data.len());
```

### Writing

```rust,no_run
use mrc::create;

let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<i16>()                 // compile-time: writer expects i16 data
    .finish()?;

// write_block_as auto-converts f32 → i16 (clamped)
writer.write_block_as(&mrc::VoxelBlock::new(
    [0, 0, 0], [512, 512, 1],
    vec![0.0f32; 512 * 512],
)?)?;

writer.update_header_stats()?;    // fills dmin/dmax/dmean/rms
writer.finalize()?;               // **required** — rewrites header
```

### Reading Extended Metadata

```rust,no_run
use mrc::ExtHeaderData;

// Auto-detect and parse extended headers
match reader.parse_extended_header() {
    ExtHeaderData::Fei1(records) => {
        println!("{} tilt images", records.len());
        println!("first tilt: {:.1}°, defocus {:.1}µm",
            records[0].alpha_tilt, records[0].defocus);
    }
    ExtHeaderData::Seri(records) => {
        println!("SerialEM series, first tilt {:.1}°",
            records[0].alpha_tilt);
    }
    ExtHeaderData::None => println!("No extended header"),
    _ => {}
}
```

---

## CLI Tools

| Binary | Install | What it does |
|---|---|---|
| `mrc-validate` | `cargo install mrc --bin mrc-validate` | Header validation, stats cross-check, NaN/Inf scan |
| `mrc-header` | `cargo install mrc --bin mrc-header` | Human-readable header dump with semantic interpretation |
| `mrc-invert` | `cargo install mrc --bin mrc-invert` | Contrast inversion v → −v, updates header stats |

---

## Further Reading

| Resource | What you'll find |
|---|---|
| [docs.rs/mrc](https://docs.rs/mrc) | Complete API reference with runnable examples on every method |
| [APIs.md](APIs.md) | Local API surface overview (offline-friendly) |
| [roadmap.md](roadmap.md) | Release history and planned features |
| [AGENTS.md](AGENTS.md) | Code organization & conventions for contributors |
| [mrcfile-official.md](mrcfile-official.md) | The MRC-2014 specification |
| [update.md](update.md) | Per-release changelogs |

---

## License

MIT — see the [LICENSE](LICENSE) file.

## Acknowledgments

- [CCP-EM](https://www.ccpem.ac.uk/) for the MRC-2014 specification
- EMDB for providing real-world test data
- The cryo-EM community for invaluable feedback
