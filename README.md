# mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc)
[![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)

> **Type-safe MRC-2014 file format reader/writer for Rust**  
> SIMD-accelerated, memory-mapped, zero-copy I/O for cryo-EM and structural biology.

---

## Overview

`mrc` is a high-performance Rust library for reading and writing MRC-2014 files — the standard format in cryo-electron microscopy (cryo-EM) and structural biology. It handles the full format specification: all data modes, compression, extended headers, volume stacks, and the quirks of real-world microscope files.

**Key features:**

- **Type-safe** — compile-time mode checking via the `Voxel` trait prevents byte mis-interpretation
- **Fast** — SIMD-accelerated conversions (AVX2, NEON), parallel encoding, zero-copy mmap access
- **Auto-conversion** — `reader.convert::<f32>()` reads any mode as `f32` without boilerplate
- **Compression** — transparent gzip/bzip2 auto-detection with configurable decompression bomb protection
- **Extended headers** — typed parsers for FEI1/2, CCP4, MRCO, SerialEM, Agard, and IMOD metadata
- **Permissive mode** — opens real-world files that other libraries reject (EPU, IMOD quirks)
- **CLI tools** — validate, inspect, and process MRC files from the command line

---

## Installation

```toml
[dependencies]
mrc = "0.4"
```

### Feature flags

| Feature | Default | What it enables |
|---------|---------|-----------------|
| `mmap` | ✅ | Memory-mapped readers and writers |
| `simd` | ✅ | AVX2/NEON accelerated integer↔f32, f16↔f32, byte-swap, stats |
| `parallel` | ✅ | Parallel encoding via `rayon` |
| `f16` | ✅ | Half-precision float (Mode 12) support |
| `gzip` | ✅ | Gzip auto-detection and I/O |
| `bzip2` | ❌ | Bzip2 auto-detection and I/O |
| `ndarray` | ❌ | Return volumes as `ndarray::Array3` via `to_ndarray()` |
| `serde` | ❌ | Serialize/Deserialize for public types |

---

## Quick Start

```rust,no_run
use mrc::{open, create, VoxelBlock};

// Read — auto-detects gzip/bzip2, handles EPU microscope quirks
let reader = open("tiltseries.mrc")?;
println!("{}×{}×{} voxels, mode {:?}",
    reader.shape().nx, reader.shape().ny, reader.shape().nz,
    reader.mode());

for slice in reader.convert::<f32>().slices() {
    let block = slice?;  // VoxelBlock<f32> — one Z-plane
}

// Write — always call .finalize() or the header stays stale
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;

writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [512, 512, 1], vec![0.0f32; 512 * 512],
)?)?;

writer.update_header_stats()?;  // fill dmin/dmax/dmean/rms
writer.finalize()?;             // rewrite header with final metadata
```

---

## Examples

### Reading with automatic mode conversion

```rust,no_run
use mrc::Reader;

let reader = Reader::open("density.mrc")?;

// Auto-convert any mode to f32 (supports i8, i16, u16, f16, Packed4Bit, complex)
for slice in reader.convert::<f32>().slices() {
    let block = slice?;
    println!("slice {} mean: {:.2}", block.offset[2],
        block.data.iter().sum::<f32>() / block.data.len() as f32);
}

// Read full volume at once
let volume = reader.convert::<f32>().read_volume()?;
println!("read {} voxels", volume.data.len());
```

### Reading extended header metadata

```rust,no_run
use mrc::{Reader, ExtHeaderData};

let reader = Reader::open("tiltseries.mrc")?;

// Auto-dispatch based on EXTTYP field
match reader.parse_extended_header() {
    ExtHeaderData::Fei1(records) => {
        for r in &records {
            println!("tilt {:.1}°, defocus {:.1} µm",
                r.alpha_tilt, r.defocus);
        }
    }
    ExtHeaderData::Ccp4(records) => {
        for r in &records {
            println!("symmetry: {}", r.as_str());
        }
    }
    ExtHeaderData::Seri(records) => {
        println!("first tilt: {:.1}°", records[0].alpha_tilt);
    }
    _ => println!("Unrecognized extended header format"),
}

// Or use typed convenience methods
if let Some(imod) = reader.imod_metadata() {
    println!("IMOD type: {:?}, tilt increment: {:.1}°",
        imod.image_type, imod.tilt_increment);
}
```

### Volume stack iteration

```rust,no_run
use mrc::Reader;

let reader = Reader::open("averages.mrc")?;

// ISPG 401–630, each sub-volume is mz slices thick
for volume in reader.volumes::<f32>()? {
    let vol = volume?;
    println!("sub-volume at z={}: {}×{}×{} voxels",
        vol.offset[2], vol.shape[0], vol.shape[1], vol.shape[2]);
}
```

### Memory-mapped reader (zero-copy for large files)

```rust,no_run
use mrc::MmapReader;

let reader = MmapReader::open("large_volume.mrc")?;

// Zero-copy typed access (native endian, matching type only)
let slab: &[f32] = reader.slab_as::<f32>(0, 1)?;
println!("first slice: {} voxels", slab.len());

// Generic iteration (always allocates per block)
for slice in reader.slices::<f32>() {
    let block = slice?;
    // process block.data
}
```

### Writing with automatic type conversion

```rust,no_run
use mrc::{create, VoxelBlock};

let mut writer = create("int16_output.mrc")
    .shape([256, 256, 128])
    .mode::<i16>()  // file stores Int16 on disk
    .finish()?;

// write_block_as auto-converts f32→i16 with clamping
let data = vec![0.0f32; 256 * 256];
writer.write_block_as(&VoxelBlock::new(
    [0, 0, 0], [256, 256, 1], data,
)?)?;

writer.finalize()?;
```

### Permissive mode (open problematic files)

```rust,no_run
use mrc::Reader;

// Open files with non-standard headers (EPU, IMOD, etc.)
let (reader, warnings) = Reader::open_permissive("legacy.mrc")?;
for w in &warnings {
    eprintln!("note: {w}");
}
```

---

## CLI Tools

Three command-line tools are included:

| Binary | Install | Description |
|--------|---------|-------------|
| `mrc-validate` | `cargo install mrc --bin mrc-validate` | Comprehensive file validation (header, stats, NaN/Inf scan) |
| `mrc-header` | `cargo install mrc --bin mrc-header` | Human-readable header inspector |
| `mrc-invert` | `cargo install mrc --bin mrc-invert` | Contrast inverter (v → −v) |

```bash
# Validate a file
mrc-validate protein.mrc

# Inspect header
mrc-header --permissive legacy.mrc

# Invert contrast (reads any mode, writes Float32)
mrc-invert density.mrc inverted.mrc
```

---

## Performance

`mrc` uses runtime CPU feature detection to select the fastest available code path:

| Operation | Scalar | AVX2 (x86_64) | NEON (AArch64) |
|-----------|--------|---------------|----------------|
| i16 → f32 | 1× | ~5× | ~4× |
| i8 → f32 | 1× | ~6× | ~5× |
| f16 → f32 | 1× | ~4× (F16C) | ~3× (fp16) |
| f32 → i16 | 1× | ~5× | ~4× |
| f32 stats | 1× | ~4× | ~3× |
| byte-swap (2/4/8) | 1× | ~6× | ~5× |

All SIMD paths fall back to scalar code when the required ISA is unavailable. No `unsafe` in the public API.

---

## Data Mode Support

| Mode | Rust type | `Voxel` impl | Typical use |
|------|-----------|--------------|-------------|
| 0 (Int8) | `i8` | ✅ | Binary masks |
| 1 (Int16) | `i16` | ✅ | Raw cryo-EM density |
| 2 (Float32) | `f32` | ✅ | Processed/reconstructed density |
| 3 (Int16Complex) | `Int16Complex` | ✅ | Complex data (obsolete) |
| 4 (Float32Complex) | `Float32Complex` | ✅ | Complex data |
| 6 (Uint16) | `u16` | ✅ | Segmentation labels |
| 12 (Float16) | `f16` | ✅ | Half-precision storage |
| 101 (Packed4Bit) | `u8` (via `slices_u8`) | ❌ | 4-bit packed; use `convert::<f32>()` |

---

## Documentation

| Resource | Description |
|----------|-------------|
| **[docs.rs/mrc](https://docs.rs/mrc)** | Full API reference with examples |
| **[APIs.md](APIs.md)** | Local API surface overview |
| **[update.log](update.log)** | Detailed changelogs for all releases |
| **[mrcfile-official.md](mrcfile-official.md)** | MRC-2014 format specification |

---

## License

MIT — see the [LICENSE](LICENSE) file.

## Acknowledgments

- [CCP-EM](https://www.ccpem.ac.uk/) for the MRC-2014 specification
- EMDB for providing real-world test data
- The cryo-EM community for invaluable feedback
