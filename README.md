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

Three lines to read any MRC file. Three more to write one.

```rust,no_run
use mrc::{open, create, VoxelBlock};

// Read — auto-detects gzip/bzip2, handles quirky headers
let reader = open("density.mrc")?;
let volume: VoxelBlock<f32> = reader.convert::<f32>().read_volume()?;
println!("{}×{}×{} = {} voxels",
    volume.shape[0], volume.shape[1], volume.shape[2], volume.data.len());

// Write — type-safe, compile-time mode checking
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;
writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [512, 512, 256], vec![0.0f32; 512 * 512 * 256],
)?)?;
writer.update_header_stats()?;  // compute dmin/dmax/dmean/rms
writer.finalize()?;             // rewrites header — always call this
```

---

## Power & Simplicity at a Glance

The `mrc` API is designed so that **common operations are one-liners** and **complex workflows read naturally**.

| What you want | How you write it |
|---|---|
| Open any MRC file (plain / gzip / bzip2) | `Reader::open("file.mrc")?` |
| Read the whole volume as `f32` | `reader.convert::<f32>().read_volume()?` |
| Read a sub-region | `reader.subregion::<f32>([x, y, z], [sx, sy, sz])?` |
| Iterate Z-slices | `reader.slices::<f32>()` → `for slice in ...` |
| Iterate batches of 16 Z-planes | `reader.slabs::<f32>(16)` → `for slab in ...` |
| Iterate 3D tiles | `reader.tiles::<f32>([64, 64, 64])?` → `for tile in ...` |
| Create a new file | `create("out.mrc").shape([512, 512, 256]).mode::<f32>().finish()?` |
| Write with auto-conversion (f32 → i16) | `writer.write_block_as(&f32_block)?` |
| Parse tilt-series metadata | `reader.fei1_metadata()` or `reader.parse_extended_header()` |
| Validate a file | `validate_full("file.mrc", false)?` |
| Open a quirky file | `Reader::open_permissive("broken.mrc")?` |
| Zero-copy mmap access | `reader.slab_as::<f32>(z, k)?` |

**No trait imports required.** Every one of these is an inherent method — no `use SomeTrait` needed.


## Installation

```toml
[dependencies]
mrc = "0.5"
```

Enable optional features in `Cargo.toml`:

```toml
mrc = { version = "0.5", features = ["ndarray", "serde", "bzip2"] }
```

For the CLI binary (`mrc` command), install the companion crate:

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
| `ndarray` | ❌ | Return volumes as `ndarray::Array3<T>` |
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

// Iterate — slices, slabs, tiles, or subregion
for slice in reader.slices::<f32>() {          // one Z-plane
    let block = slice?;                        // VoxelBlock<f32>
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

// Any sub-region by coordinate
let patch = reader.subregion::<f32>([10, 10, 5], [32, 32, 8])?;
```

### Auto-conversion — read any MRC mode as `f32`

Don't care whether the file is Int8, Int16, Uint16, Float16, or even Packed4Bit? Use `convert::<f32>()` and the crate handles the rest.

```rust,no_run
// Read any file mode as f32 — Int16, Uint16, Float16, even Packed4Bit
for slice in reader.convert::<f32>().slices() {
    let block: mrc::VoxelBlock<f32> = slice?;
    println!("slice {}: mean = {:.2}",
        block.offset[2],
        block.data.iter().sum::<f32>() / block.data.len() as f32);
}

// Or read the whole converted volume in one call
let block = reader.convert::<f32>().read_volume()?;
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
use mrc::{create, Compression};

// Gzip-compressed output — same API, just finish_gzip()
let mut writer = create("output.mrc.gz")
    .shape([256, 256, 128])
    .mode::<f32>()
    .compression(Compression::Best)
    .finish_gzip()?;
writer.write_block(&mrc::VoxelBlock::new(
    [0, 0, 0], [256, 256, 128], vec![0.0f32; 256 * 256 * 128],
)?)?;
writer.finalize()?;  // compresses & writes to disk
```

### Memory-mapped I/O — zero-copy for large files

Files too large for RAM? `Reader::open` automatically uses memory-mapped I/O (requires `mmap` feature). The OS pages data on demand.

```rust,no_run
let reader = Reader::open("huge_volume.mrc")?;

// Zero-copy typed access to Z-planes
let slab: &[f32] = reader.slab_as::<f32>(0, 1)?;  // no allocation
println!("first plane has {} voxels", slab.len());
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
for result in reader.volumes::<f32>()? {
    let vol = result?;
    println!("sub-volume at z={}: {}×{}×{}",
        vol.offset[2], vol.shape[0], vol.shape[1], vol.shape[2]);
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

The [`mrc-cli`](https://crates.io/crates/mrc-cli) crate provides the `mrc`
command-line tool with subcommands for inspection, validation, conversion,
PNG/GIF export, and resampling.

```bash
cargo install mrc-cli
mrc info protein.mrc
mrc header density.mrc
mrc validate tiltseries.mrc
mrc stats protein.mrc
mrc invert input.mrc output.mrc
mrc convert input.mrc output.mrc --mode i16
mrc slice volume.mrc -z 42 -o slice.mrc
mrc crop volume.mrc -o roi.mrc --x 100 --y 100 --z 50 -s 128,128,64
mrc unstack tiltseries.mrc -o frame
mrc rescale volume.mrc output.mrc --down 2
mrc png volume.mrc -z 0 -o slice.png
mrc movie volume.mrc -o movie.gif --pingpong
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
