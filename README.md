# mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc)
[![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)
[![CI](https://github.com/elemeng/mrc/workflows/CI/badge.svg)](https://github.com/elemeng/mrc/actions)

> **Type-safe MRC-2014 reader/writer for Rust** — SIMD-accelerated, mmap-enabled, with full cryo-EM metadata support.

A type-safe Rust encoder/decoder for MRC files — the standard format in cryo-electron microscopy and structural biology. Automatically handles endianness, type conversion, and compression with SIMD acceleration, exposing a powerful yet intuitive and friendly read/write API so you can focus on your data.

---

## Quick Start

One line to read any MRC file. One line to write one.

```rust,no_run
use mrc::{read_as, write_as};

// Read — auto-detects gzip/bzip2, handles quirky headers
let (header, data): (_, Vec<f32>) = read_as("density.mrc")?;
println!("{}×{}×{} = {} voxels",
    header.nx, header.ny, header.nz, data.len());

// Write — type-safe, single call
write_as("output.mrc", &data, [512, 512, 256])?;
```

---

## Power & Simplicity at a Glance

The `mrc` API is designed so that **common operations are one-liners** and **complex workflows read naturally**.

| What you want | How you write it |
|---|---|
| Open any MRC file (plain / gzip / bzip2) | `Reader::open("file.mrc")?` |
| One-shot read (open + read_volume) | `let (h, d): (_, Vec<f32>) = read_as("file.mrc")?;` |
| One-shot write (create + write + finalize) | `write_as("out.mrc", &data, [512, 512, 256])?;` |
| Read the whole volume as `f32` | `reader.convert::<f32>().read_volume()?` |
| Read a sub-region | `reader.subregion([x, y, z], [sx, sy, sz])?` |
| Iterate Z-slices | `reader.slices()` → `for slice in ...` |
| Iterate sub-volumes in a stack | `reader.volumes()?` → `for vol in ...` |
| Create a new file | `create("out.mrc").shape([512, 512, 256]).mode::<f32>().finish()?` |
| Write with auto-conversion (f32 → i16) | `writer.write_block_as(&f32_block)?` |
| Parse tilt-series metadata | `reader.fei1_metadata()` or `reader.parse_extended_header()` |
| Validate a file | `validate_full("file.mrc", false)?` |
| Open a quirky file | `Reader::open_permissive("broken.mrc")?` |

**No trait imports required.** Every one of these is an inherent method — no `use SomeTrait` needed.


## Installation

```toml
[dependencies]
mrc = "0.6"
```

Enable optional features in `Cargo.toml`:

```toml
mrc = { version = "0.6", features = ["ndarray", "serde", "bzip2"] }
```

For the `mrc-cli` binary, install the companion crate:

```bash
cargo install mrc-cli
```

| Feature | Default | What it adds |
|---|---|---|
| `mmap` | ✅ | Memory-mapped I/O (auto-selected for large files) |
| `f16` | ✅ | Half-precision float (`half::f16`) support |
| `simd` | ✅ | AVX2/NEON acceleration |
| `parallel` | ✅ | Parallel encoding via `rayon` |
| `gzip` | ✅ | Gzip auto-detection and compressed writer |
| `bzip2` | ❌ | Bzip2 auto-detection and compressed writer |
| `ndarray` | ❌ | Return volumes as `ndarray::Array3<T>` via `to_ndarray()` |
| `serde` | ❌ | Serialize/Deserialize for all public types |

---

## Quick Tour

> See [docs.rs/mrc](https://docs.rs/mrc) for the full API documentation, runnable examples, and detailed guidance. The examples below are just a few highlights.

### Reading — any file, any mode, any shape

```rust,no_run
use mrc::Reader;

// Open — auto-detects compression and byte order
let reader = Reader::open("tiltseries.mrc")?;
println!("{}×{}×{} voxels, mode {:?}",
    reader.shape().nx, reader.shape().ny, reader.shape().nz,
    reader.mode());

// Check the file's mode at runtime and dispatch accordingly:
match reader.mode() {
    mrc::Mode::Float32 => { /* process f32 slices */ }
    mrc::Mode::Int16   => { /* process i16 slices */ }
    mrc::Mode::Uint16  => { /* process u16 slices */ }
    mrc::Mode::Int8    => { /* process i8 slices */ }
    other              => todo!("mode {other:?}"),
}

// Or just use slices() and match DataView:
for slice in reader.slices() {
    let block = slice?;
    match block.data() {
        mrc::DataView::Float32(data) => { /* process &[f32] */ }
        mrc::DataView::Int16(data)   => { /* process &[i16] */ }
        _ => {}
    }
}

// Full volume in one call
let block = reader.read_volume()?;
let DataView::Float32(data) = block.data() else { panic!("expected Float32") };
println!("{} voxels", data.len());

// Any sub-region by coordinate
let block = reader.subregion([10, 10, 5], [32, 32, 8])?;
let DataView::Float32(patch) = block.data() else { panic!("expected Float32") };
```

### Auto-conversion — read any MRC mode as `f32`

Don't care whether the file is Int8, Int16, Uint16, Float16, or even Packed4Bit?
Use `convert::<f32>()` and the crate handles the rest — or match on `reader.mode()`
to handle each type individually.

```rust,no_run
// Option A: auto-convert everything to f32 in one call
for slice in reader.convert::<f32>().slices() {
    let block: mrc::VoxelBlock<f32> = slice?;
    println!("slice {}: mean = {:.2}",
        block.offset[2],
        block.data.iter().sum::<f32>() / block.data.len() as f32);
}

// Option B: use default reader methods and match on DataView
match reader.mode() {
    mrc::Mode::Int16 => {
        for slice in reader.slices() {
            let block = slice?;
            let DataView::Int16(data) = block.data() else { panic!("expected Int16") };
            println!("i16 slice, min={}", data.iter().min().unwrap());
        }
    }
    mrc::Mode::Float32 => {
        let block = reader.read_volume()?;
        let DataView::Float32(data) = block.data() else { panic!("expected Float32") };
        println!("f32 volume: {} voxels", data.len());
    }
    _ => { /* handle other modes */ }
}
// Or read the whole converted volume in one call
let block = reader.convert::<f32>().read_volume()?;

// The same converter also supports slabs, tiles, subregion,
// with_complex_strategy, with_m0_interpretation, and to_ndarray().
```

### Writing — type-safe, flexible, fast

```rust,no_run
use mrc::create;

// Create a Float32 file
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;

// Write one slice at a time
for z in 0..256 {
    let slice = vec![0.0f32; 512 * 512];
    writer.write_block(&mrc::VoxelBlock::new(
        [0, 0, z], [512, 512, 1], slice,
    )?)?;
}

// Or write with auto-conversion: f32 data → i16 file
writer.write_block_as(&mrc::VoxelBlock::new(
    [0, 0, 0], [512, 512, 1],
    vec![0.0f32; 512 * 512],
)?)?;

// Write in parallel (requires `parallel` feature)
writer.write_block_parallel(&mrc::VoxelBlock::new(
    [0, 0, 0], [512, 512, 256], vec![0.0f32; 512 * 512 * 256],
)?)?;

writer.update_header_stats()?;    // fills dmin/dmax/dmean/rms
writer.finalize()?;               // **required** — rewrites header
```

### Writing compressed files

```rust,no_run
use mrc::{create, CompressionLevel};

// Gzip-compressed output — same API, just finish_gzip()
let mut writer = create("output.mrc.gz")
    .shape([256, 256, 128])
    .mode::<f32>()
    .compression(CompressionLevel::Best)
    .finish_gzip()?;
writer.write_block(&mrc::VoxelBlock::new(
    [0, 0, 0], [256, 256, 128], vec![0.0f32; 256 * 256 * 128],
)?)?;
writer.finalize()?;  // compresses & writes to disk
```

### Memory-mapped I/O — zero-copy for large files

Files too large for RAM? `Reader::open` automatically uses memory-mapped I/O (requires `mmap` feature). The OS pages data on demand. The default reader methods return `DataBlock` views that borrow directly from the mapped memory.

```rust,no_run
let reader = Reader::open("huge_volume.mrc")?;

// Default methods return DataBlock with zero-copy DataView
for slice in reader.slices() {
    let block = slice?;
    let DataView::Float32(data) = block.data() else { continue; };
    println!("plane with {} voxels (zero-copy)", data.len());
}
```

### Reading Extended Metadata — one method call

```rust,no_run
use mrc::ExtHeaderData;

// Auto-detect and parse whatever extended header the file has
match reader.parse_extended_header() {
    ExtHeaderData::Fei1(records) => {
        println!("FEI1 tilt series ({} images)", records.len());
        println!("first: tilt {:.1}°, defocus {:.1}µm",
            records[0].alpha_tilt, records[0].defocus);
    }
    ExtHeaderData::Fei2(records) => {
        println!("FEI2 — {} records", records.len());
    }
    ExtHeaderData::Ccp4(records) => {
        println!("CCP4 symmetry — {} records", records.len());
    }
    ExtHeaderData::Seri(records) => {
        println!("SerialEM — first tilt {:.1}°", records[0].alpha_tilt);
    }
    ExtHeaderData::None => println!("No extended header"),
    _ => {}
}

// Or use typed convenience methods directly
if let Some(records) = reader.fei1_metadata() {
    println!("{} FEI1 records", records.len());
}
if let Some(imod) = reader.imod_metadata() {
    println!("IMOD: {:?}, tilt increment {:.1}°",
        imod.image_type, imod.tilt_increment);
}
```

### Volume stacks — iterate sub-volumes

Volume stacks (ISPG 401–630) pack multiple sub-volumes in one file.

```rust,no_run
for result in reader.volumes()? {
    let vol = result?;
    println!("sub-volume at z={}: {}×{}×{}",
        vol.offset()[2], vol.shape()[0], vol.shape()[1], vol.shape()[2]);
}
```

### Validation — catch issues early

```rust,no_run
use mrc::validate::{validate_full, Severity};

let report = validate_full("protein.mrc", false)?;
if !report.is_valid() {
    for issue in &report.issues {
        if issue.severity == Severity::Error {
            eprintln!("[{}] {}", issue.category, issue.message);
        }
    }
}
```

### Working with quirky files

Common microscope quirks (NVERSION left at 0, `"MAP\0"` instead of `"MAP "`) are handled transparently by `open()`. For truly broken files, permissive mode turns non-critical errors into warnings:

```rust,no_run
let (reader, warnings) = Reader::open_permissive("legacy.mrc")?;
if reader.is_truncated() {
    eprintln!("warning: file is incomplete");
}
for w in &warnings { eprintln!("note: {w}"); }
```

### Real-world workflow — the full pipeline

```rust,no_run
use mrc::{open, create, VoxelBlock};

// 1. Open a tilt series from any microscope format
let reader = open("tiltseries.mrc")?;
println!("{}×{}×{}, mode {:?}",
    reader.shape().nx, reader.shape().ny, reader.shape().nz,
    reader.mode());

// 2. Read FEI metadata (or CCP4, SerialEM, Agard...)
if let Some(records) = reader.fei1_metadata() {
    for (i, r) in records.iter().enumerate() {
        println!("tilt {i}: α={:.1}°, defocus={:.1} µm",
            r.alpha_tilt, r.defocus);
    }
}

// 3. Process each slice as f32 (auto-converts from any mode)
for slice in reader.convert::<f32>().slices() {
    let block = slice?;
    // block.data: Vec<f32> — ready for filtering, CTF, alignment
}

// 4. Write the reconstructed volume
let mut writer = create("reconstructed.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;
writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [512, 512, 256], processed_data,
)?)?;
writer.update_header_stats()?;
writer.finalize()?;
```

---

## CLI Tools

The [`mrc-cli`](https://crates.io/crates/mrc-cli) crate provides the `mrc-cli`
command-line tool with subcommands for inspection, validation, conversion,
PNG/GIF export, and resampling.

```bash
cargo install mrc-cli
mrc-cli info protein.mrc
mrc-cli header density.mrc
mrc-cli validate tiltseries.mrc
mrc-cli stats protein.mrc
mrc-cli invert input.mrc output.mrc
mrc-cli convert input.mrc output.mrc --mode i16
mrc-cli slice volume.mrc -z 42 -o slice.mrc
mrc-cli crop volume.mrc -o roi.mrc --x 100 --y 100 --z 50 -s 128,128,64
mrc-cli unstack tiltseries.mrc -o frame
mrc-cli rescale volume.mrc output.mrc --down 2
mrc-cli png volume.mrc -z 0 -o slice.png
mrc-cli movie volume.mrc -o movie.gif --pingpong
```

See the [`mrc-cli` crate on crates.io](https://crates.io/crates/mrc-cli) for the
full command reference and examples.

## Further Reading

| Resource | What you'll find |
|---|---|
| [docs.rs/mrc](https://docs.rs/mrc) | Complete API reference with runnable examples on every method |
| [APIs.md](APIs.md) | Local API surface overview (offline-friendly) |
| [mrc-cli on crates.io](https://crates.io/crates/mrc-cli) | CLI binary reference and examples |
| [roadmap.md](roadmap.md) | Release history and planned features |
| [AGENTS.md](AGENTS.md) | Code organization & conventions for contributors |
| [mrcfile-official.md](mrcfile-official.md) | The MRC-2014 specification |
| [update.md](update.md) | Per-release changelogs |

---

## Acknowledgments

- [CCP-EM](https://www.ccpem.ac.uk/) for the MRC-2014 specification
- [EMDB](https://www.ebi.ac.uk/emdb/) for providing real-world test data
- The cryo-EM community for invaluable feedback

## Contributing

**Contributions are welcome — whatever your skill level.**

This crate is built by and for the cryo-EM community. Whether you're fixing a typo, adding a test, implementing a new feature, or just asking a question, your input makes the project better.

- **Report bugs** — open an issue with steps to reproduce
- **Request features** — what format feature or workflow is missing from your pipeline?
- **Submit PRs** — see [AGENTS.md](AGENTS.md) for code organization and conventions
- **Improve docs** — better examples, clearer explanations, fix typos
- **Share real files** — MRC files with unusual extended headers or edge cases help us test

All contributions are subject to the [MIT License](LICENSE).

---

*Format specs come and go, but cryo-EM data is forever — make yours readable by the next generation of tools.*

MIT — see the [LICENSE](LICENSE) file.
