# 🧬 mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc) [![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)

> **Type-safe MRC-2014 file format reader/writer for Rust**

A high-performance, memory-efficient library for reading and writing MRC (Medical Research Council) format files used in cryo-electron microscopy and structural biology. Designed for scientific computing with safety and performance as top priorities.

## ✨ Why mrc?

- **🚀 Iterator-centric**: Stream slices, slabs, or tiles on demand
- **⚡ SIMD-accelerated**: AVX2/NEON for the common i16→f32 path
- **🔒 Type-safe I/O**: Compile-time mode matching prevents silent data corruption
- **🗺️ Memory-mapped I/O**: `MmapReader` / `MmapWriter` for files larger than RAM
- **📦 Compression**: Auto-detect and read gzip / bzip2 MRC files
- **🏷️ FEI metadata**: Structured parsing of FEI1/FEI2 extended headers

**Note: This crate is currently under active development. While most features are functional, occasional bugs and API changes are possible. Contributions are welcome—please report issues and share your ideas!**

## 📦 Installation

```toml
[dependencies]
mrc = "0.2"

# For all features (defaults are usually sufficient)
mrc = { version = "0.2", features = ["mmap", "f16", "simd", "parallel", "gzip"] }
```

## 🚀 Quick Start

### Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌────────────────┐
│   File System   │────▶│  Header Parsing  │────▶│  Iterator API  │
│ (.mrc/.mrc.gz)  │     │   (1024 bytes)   │     │  (Zero-copy)   │
└─────────────────┘     └──────────────────┘     └────────────────┘
         │                       │                       │
   ┌─────────────┐          ┌────────┐              ┌─────────┐
   │ Reader      │          │ Header │              │ VoxelBlock
   │ MmapReader  │          │        │              │         │
   │ Writer      │          └────────┘              └─────────┘
   │ MmapWriter  │
   │ GzipWriter  │
   │ Bzip2Writer │
   └─────────────┘
```

### MRC File Structure

```text
| 1024 bytes | NSYMBT bytes | data_size bytes |
|  header    | ext header   | voxel data      |
```

### 📖 Reading MRC Files

```rust
use mrc::open;

fn main() -> Result<(), mrc::Error> {
    // Open an MRC file - auto-detects plain, gzip, or bzip2
    let reader = open("protein.mrc")?;

    // Get volume dimensions
    let shape = reader.shape();
    println!("Volume: {}×{}×{} voxels", shape.nx, shape.ny, shape.nz);

    // Iterate over slices
    for slice in reader.slices::<f32>() {
        let block = slice?;  // VoxelBlock<f32>
        println!("Slice {}: {} voxels", block.offset[2], block.len());
    }

    // Or read with automatic conversion to f32 (common cryo-EM workflow)
    for slice in reader.slices_f32() {
        let block = slice?;
        let sum: f32 = block.data.iter().sum();
        println!("Slice sum: {}", sum);
    }

    Ok(())
}
```

### ✏️ Creating New Files

```rust
use mrc::{create, VoxelBlock};

fn main() -> Result<(), mrc::Error> {
    // Create a new file with the builder pattern
    let mut writer = create("output.mrc")
        .shape([512, 512, 256])
        .mode::<f32>()
        .finish()?;

    // Write voxel data slice by slice
    for z in 0..256 {
        let block = VoxelBlock::new(
            [0, 0, z],
            [512, 512, 1],
            vec![0.0f32; 512 * 512],
        )?;
        writer.write_block(&block)?;
    }

    // Finalize rewrites the header to disk.
    // Note: dmin/dmax/dmean/rms are NOT updated automatically.
    // Call writer.update_header_stats() if needed.
    writer.finalize()?;
    Ok(())
}
```

## ⚠️ Migrating from v0.1

v0.2 is a complete architectural redesign. Key API changes:

| v0.1 | v0.2 |
|------|------|
| `MrcView::new(data)` | `Reader::open(path)` / `open(path)` |
| `MrcFile::create(path, header)` | `create(path).shape(dims).mode::<T>().finish()` |
| `MrcView::view::<f32>()` | `reader.slices::<f32>()` |
| `MrcViewMut` | `Writer` + `VoxelBlock<T>` |
| `MrcMmap` | `MmapReader` / `MmapWriter` |

**Migration example:**

```rust
// v0.1: Load entire file into memory
let data = std::fs::read("file.mrc")?;
let view = MrcView::new(data)?;
let floats = view.view::<f32>()?;

// v0.2: Stream with iterators
let reader = open("file.mrc")?;
for slice in reader.slices::<f32>() {
    let block = slice?;
    // process block.data
}
```

**New in v0.2:** SIMD acceleration, parallel encoding, type conversion iterators, `MmapReader`, `MmapWriter`, compression support, unified reader API, FEI extended header parsing.

## 🗺️ API Overview

### Core Types

| Type | Purpose | Example |
|------|---------|---------|
| [`Reader`] | Auto-detect compression | [`Reader::open`] / [`open()`] |
| [`Reader`] | Read plain MRC files | `Reader::open("file.mrc")?` |
| [`MmapReader`] | Memory-mapped reading | `MmapReader::open("large.mrc")?` |
| [`Writer`] | Write MRC files | `create("out.mrc").shape([64,64,64]).mode::<f32>().finish()?` |
| [`MmapWriter`] | Memory-mapped writing | `create("out.mrc").shape(...).finish_mmap()?` |
| [`WriterBuilder`] | Configure new files | `create(path).shape(dims).mode::<T>()` |
| [`Header`] | 1024-byte MRC header | `Header::new()` |
| [`HeaderBuilder`] | Fluent header construction | `HeaderBuilder::new().shape([64,64,64]).mode::<f32>().build()?` |
| [`Mode`] | Data type enumeration | `Mode::Float32` |
| [`VoxelBlock<T>`] | Chunk of voxel data | `VoxelBlock::new(offset, shape, data)` |
| [`VolumeShape`] | Volume dimensions | `VolumeShape::new(nx, ny, nz)` |
| [`GzipWriter`] | Gzip-compressed writer | `create("out.mrc.gz").shape(dims).mode::<T>().finish_gzip()?` |
| [`Bzip2Writer`] | Bzip2-compressed writer | `create("out.mrc.bz2").shape(dims).mode::<T>().finish_bzip2()?` |
| [`Fei1Metadata`] | FEI1 extended metadata | `Fei1Metadata::from_bytes(bytes)` |
| [`Fei2Metadata`] | FEI2 extended metadata | `Fei2Metadata::from_bytes(bytes)` |

### Iterator API

All reader types provide a unified iterator API directly:

```rust

// Iterate over individual slices (Z axis)
for slice in reader.slices::<f32>() {
    let block = slice?;
    // Process slice
}

// Iterate over slabs (multiple slices at once)
for slab in reader.slabs::<f32>(10) {  // 10 slices per slab
    let block = slab?;
    // Process slab
}

// Iterate over arbitrary 3D tiles
for tile in reader.tiles::<f32>([64, 64, 64]) {
    let block = tile?;
    // Process 64³ tile
}

// Semantic aliases
for image in reader.images::<f32>() { /* same as slices() */ }
for plane in reader.planes::<f32>() { /* same as slices() */ }
for stack in reader.image_stack::<f32>(10) { /* same as slabs(10) */ }
for stack in reader.plane_stack::<f32>(10) { /* same as slabs(10) */ }
for vol in reader.volumes::<f32>()? { /* full volumes from a stack */ }
```

### Direct Access

```rust
// Read a specific subregion directly
let block = reader.subregion::<f32>([0, 0, 0], [64, 64, 64])?;
```

### Type Conversion

The crate intentionally does **not** provide generic type conversion — that is
the caller's responsibility. Only the overwhelmingly common cryo-EM workflows
are supported as conveniences:

```rust
// Read an Int16/Uint16/Int8/Float32/Float16 file as f32
for slice in reader.slices_f32()? {
    let block = slice?;
    // block.data is Vec<f32>
}

// Iterate over slabs with f32 conversion
for slab in reader.slabs_f32(10)? {
    let block = slab?;
    // block.data is Vec<f32>
}

// Convert Mode 6 (Uint16) voxels to u8
for slice in reader.slices_u8() {
    let block = slice?;
    // block.data is Vec<u8>
}

// Mode 0 (8-bit) with signed/unsigned interpretation
use mrc::M0Interpretation;
for slice in reader.slices_mode0(M0Interpretation::Signed) {
    let block = slice?;
    // block.data is Vec<f32>
}
for slab in reader.slabs_mode0(10, M0Interpretation::Unsigned) {
    let block = slab?;
    // block.data is Vec<f32>
}

// Write f32 data to a Float16 file
let mut writer = create("output.mrc")
    .shape([256, 256, 128])
    .mode::<f16>()
    .finish()?;

let f32_data: VoxelBlock<f32> = /* ... */;
writer.write_f16_from_f32(&f32_data)?;

// Write u8 data to a Uint16 (Mode 6) file (auto-widened)
let mut writer = create("seg.mrc")
    .shape([256, 256, 128])
    .mode::<u16>()
    .finish()?;
writer.write_u8_block(&VoxelBlock::new(
    [0, 0, 0], [256, 256, 1], vec![255u8; 256*256],
)?)?;
```

**Safety note:** `reader.slices::<f32>()` on an Int16 file returns
`Error::ModeMismatch` instead of silently decoding 2-byte voxels as 4-byte
floats. Use `slices_f32()` for automatic conversion.

### Compression

[`Reader::open`] (and the convenience [`open()`]) automatically detects gzip and bzip2
compression from the file magic bytes:

```rust
use mrc::open;

// Works for plain .mrc, .mrc.gz, and .mrc.bz2
let reader = open("protein.mrc")?;
```

You can also open compressed files directly:

```rust
use mrc::Reader;

let reader = Reader::open_gzip("protein.mrc.gz")?;
let reader = Reader::open_bzip2("protein.mrc.bz2")?;
```

And write compressed files:

```rust
use mrc::{create, VoxelBlock};

let mut writer = create("output.mrc.gz")
    .shape([256, 256, 128])
    .mode::<f32>()
    .finish_gzip()?;
writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [256, 256, 1], vec![0.0f32; 256*256],
)?)?;
writer.finalize()?;
```

### Memory-Mapped I/O

For large files that don't fit in RAM, memory-mapped I/O lets the OS handle
paging:

```rust
use mrc::MmapReader;

let reader = MmapReader::open("large_volume.mrc")?;

// Same iterator API as Reader
for slice in reader.slices::<f32>() {
    let block = slice?;
    // OS automatically pages data in/out
}

// Direct byte access (zero-copy)
let bytes = reader.data_bytes();  // &[u8] backed by mmap
```

Memory-mapped writes are also supported:

```rust
use mrc::create;

let mut writer = create("output.mrc")
    .shape([1024, 1024, 512])
    .mode::<f32>()
    .finish_mmap()?;

writer.write_block(&block)?;
writer.finalize()?;
```

| Use | When |
|-----|------|
| `Reader` | Small files, simple sequential access |
| `MmapReader` | Large files, memory-constrained environments, random access |

### Permissive Mode

Readers support a *permissive* open mode that collects non-fatal issues as
warnings instead of hard errors:

```rust
use mrc::Reader;

let (reader, warnings) = Reader::open_permissive("file.mrc")?;
for w in &warnings {
    eprintln!("Warning: {}", w);
}
```

This is useful for reading files from less strict sources (e.g., legacy
instruments) where the data is valid but the header has minor issues.

### Convenience Functions

```rust
use mrc::{open, create};

// Reading
let reader = open("file.mrc")?;       // auto-detect compression (plain/gzip/bzip2)

// Writing
let writer = create("out.mrc")        // standard file I/O
    .shape([64, 64, 64])
    .mode::<f32>()
    .finish()?;

let mmap_writer = create("out.mrc")   // memory-mapped (requires mmap)
    .shape([64, 64, 64])
    .mode::<f32>()
    .finish_mmap()?;
```

## 🔧 Header Construction

### Direct Header Manipulation

```rust
use mrc::Header;

let mut header = Header::new();

// Basic dimensions
header.nx = 2048;
header.ny = 2048;
header.nz = 512;

// Data type
header.mode = Mode::Float32 as i32;

// Physical dimensions in Ångströms
header.xlen = 204.8;
header.ylen = 204.8;
header.zlen = 102.4;

// Cell angles for crystallography
header.alpha = 90.0;
header.beta = 90.0;
header.gamma = 90.0;

// Extended header type (optional)
header.set_exttyp(*b"FEI1");
```

### Fluent Builder

```rust
use mrc::HeaderBuilder;

let header = HeaderBuilder::new()
    .shape([2048, 2048, 512])
    .mode::<f32>()
    .cell_lengths(204.8, 204.8, 102.4)
    .cell_angles(90.0, 90.0, 90.0)
    .ispg(1)
    .exttyp(*b"FEI1")
    .build()?;
```

### Key Header Fields

| Field | Type | Description |
|-------|------|-------------|
| `nx, ny, nz` | `i32` | Image dimensions |
| `mode` | `i32` | Data type (see Mode enum) |
| `xlen, ylen, zlen` | `f32` | Cell dimensions (Å) |
| `alpha, beta, gamma` | `f32` | Cell angles (°) |
| `mapc, mapr, maps` | `i32` | Axis mapping (1,2,3 permutation) |
| `dmin, dmax, dmean` | `f32` | Data statistics |
| `ispg` | `i32` | Space group number |
| `nsymbt` | `i32` | Extended header size |
| `origin` | `[f32; 3]` | Origin coordinates |
| `exttyp` | `[u8; 4]` | Extended header type |
| `rms` | `f32` | RMS deviation from mean |
| `nlabl` | `i32` | Number of labels (0–10) |

### Volume Type Introspection

The `Header` provides convenience methods following Python `mrcfile` conventions:

```rust
let h = header;

// Volume type checks
h.is_single_image();   // nz == 1
h.is_image_stack();    // ispg == 0
h.is_volume();         // 3D volume (ispg != 0 and not a stack)
h.is_volume_stack();   // ispg in 401–630

// Computed properties
h.voxel_size();        // [xlen/mx, ylen/my, zlen/mz] in Å/pixel
h.logical_shape();     // 4D shape following mrcfile conventions
h.get_labels();        // Vec<String> of non-empty labels
```

## 📊 Data Type Support

| [`Mode`] | Value | Rust Type | Bytes | Description | Use Case |
|----------|-------|-----------|-------|-------------|----------|
| `Int8` | 0 | `i8` | 1 | Signed 8-bit integer | Binary masks |
| `Int16` | 1 | `i16` | 2 | Signed 16-bit integer | Cryo-EM density |
| `Float32` | 2 | `f32` | 4 | 32-bit float | Standard density |
| `Int16Complex` | 3 | [`Int16Complex`] | 4 | Complex 16-bit | Phase data |
| `Float32Complex` | 4 | [`Float32Complex`] | 8 | Complex 32-bit | Fourier transforms |
| `Uint16` | 6 | `u16` | 2 | Unsigned 16-bit | Segmentation |
| `Float16` | 12 | `f16`[^1] | 2 | 16-bit float | Memory efficiency |
| `Packed4Bit` | 101 | [`Packed4Bit`] | 0.5 | Packed 4-bit[^2] | Compression |

[^1]: Requires `f16` feature. Uses the [`half`](https://docs.rs/half) crate; no nightly Rust required.
[^2]: Packed4Bit is provided for manual nibble unpacking via `first()`/`second()`. Full read/write support for Mode 101 is not yet implemented.

Complex numbers can be converted to real values via [`ComplexToRealStrategy`]:

| Strategy | Description |
|----------|-------------|
| `RealPart` | Extract the real component |
| `ImaginaryPart` | Extract the imaginary component |
| `Magnitude` | Compute `sqrt(real² + imag²)` |
| `Phase` | Compute `atan2(imag, real)` |

## 🏷️ FEI Extended Headers

This crate provides structured parsing of FEI1 and FEI2 extended headers
commonly found in cryo-EM data collected on Thermo Fisher/FEI microscopes.

```rust
use mrc::{Fei1Metadata, Fei2Metadata, parse_fei1_records, parse_fei2_records};

// After opening a file with FEI extended headers
let reader = open("tilt_series.mrc")?;
let ext_bytes = reader.ext_header_bytes();

// Parse FEI1 records (768 bytes each)
if let Some(records) = parse_fei1_records(ext_bytes) {
    for record in &records {
        println!("Dose: {} e/Å²", record.dose);
        println!("Defocus: {} µm", record.defocus);
        println!("Tilt angle: {}°", record.alpha_tilt);
    }
}

// Or parse a single record directly
if let Some(meta) = Fei1Metadata::from_bytes(ext_bytes) {
    println!("Microscope: {:?}", meta.microscope_type);
}

// FEI2 extends FEI1 with additional v2 fields (888 bytes each)
if let Some(records) = parse_fei2_records(ext_bytes) {
    for record in &records {
        println!("Scan rotation: {}", record.scan_rotation);
    }
}
```

## ⚡ Performance Features

### SIMD Acceleration

The `simd` feature (enabled by default) uses AVX2 (x86_64) or NEON (AArch64)
to accelerate the common i16→f32, u16→f32, and i8→f32 paths inside
`slices_f32()` and `slabs_f32()`. No explicit SIMD code is required in user
code.

### Zero-Copy Reading

`Reader` loads the entire file into memory (as raw bytes) and decodes slices
on demand. For memory-mapped access, use `MmapReader`:

```rust
use mrc::MmapReader;

// Memory-mapped reading — zero-copy raw byte access
let reader = MmapReader::open("data.mrc")?;

// True zero-copy typed access (native-endian files only):
let slice: &[f32] = reader.slab_as::<f32>(0, 1)?;

// Generic typed iteration (always allocates per block):
for slice in reader.slices::<f32>() {
    let block = slice?;
    // process block.data
}
```

### Parallel Writing

With the `parallel` feature, large writes use Rayon for parallel encoding:

```rust
let mut writer = create("large.mrc")
    .shape([2048, 2048, 512])
    .mode::<f32>()
    .finish()?;

// This uses parallel encoding internally
writer.write_block_parallel(&large_block)?;
writer.finalize()?;
```

### Header Statistics

The writer can compute and update header statistics after writing data:

```rust
let mut writer = create("output.mrc")
    .shape([256, 256, 128])
    .mode::<f32>()
    .finish()?;

// Write all data ...
writer.update_header_stats()?;  // updates dmin, dmax, dmean, rms
writer.finalize()?;
```

The reader can cross-check header statistics against actual data:

```rust
let reader = open("file.mrc")?;
reader.validate_header_stats()?;  // Returns Ok or StatsMismatch error
```

## 🎯 Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `mmap` | Memory-mapped I/O | ✅ |
| `f16` | Half-precision support (via `half` crate) | ✅ |
| `simd` | SIMD acceleration | ✅ |
| `parallel` | Parallel encoding | ✅ |
| `gzip` | Gzip-compressed MRC files | ✅ |
| `bzip2` | Bzip2-compressed MRC files | ❌ |

## 🛠️ CLI Tools

The crate ships two standalone binaries:

### `mrc-validate` — validation

```bash
cargo build --release --bin mrc-validate

./mrc-validate protein.mrc
./mrc-validate --permissive legacy.mrc
./mrc-validate --stats-only protein.mrc
```

Output includes compression type, header validity, data statistics
cross-check, dimensions, mode, endianness, voxel size, and labels.

### `mrc-header` — header inspection

```bash
cargo build --release --bin mrc-header

./mrc-header protein.mrc
./mrc-header --permissive legacy.mrc
```

Prints every header field with semantic interpretation: volume type
(single image / stack / volume / volume stack), axis names (X/Y/Z),
space group description, extended header type, sentinel-aware
statistics display, and validation summary.

### `mrc-invert` — contrast inversion

```bash
cargo build --release --bin mrc-invert

./mrc-invert input.mrc output.mrc
```

Negates every voxel value (v → −v) to flip black-on-white to
white-on-black and vice versa.  Reads any mode (auto-detects
compression), writes Float32 output with updated header statistics.

## 🛣️ Development Roadmap

### ✅ **Current Release (v0.2.x): Core + SIMD + FEI**

- [x] Complete MRC-2014 format support
- [x] Iterator-centric API (slices, slabs, tiles)
- [x] Type-safe I/O with compile-time mode checking
- [x] SIMD acceleration (AVX2, NEON)
- [x] Zero-copy fast paths
- [x] Parallel encoding
- [x] Memory-mapped I/O (`MmapReader`, `MmapWriter`)
- [x] All data types (modes 0–4, 6, 12, 101)
- [x] Compression support (gzip, bzip2)
- [x] Unified reader API (inherent methods on Reader / MmapReader)
- [x] FEI1/FEI2 extended header parsing
- [x] Type conversion conveniences (`slices_f32`, `slices_u8`, `slices_mode0`)
- [x] Header statistics computation and validation
- [x] `mrc-validate` CLI tool
- [x] Permissive mode for reading non-standard files
- [x] Volume stack support

### 🚧 **Next Release (v0.3.x): Extended Features**

- [ ] Extended header parsing for CCP4, MRCO, SERI, AGAR formats
- [ ] Streaming decompression (avoid loading entire compressed files into RAM)
- [ ] Dedicated benchmark suite (`criterion` in dev-deps but no `benches/` dir)

### 🚀 **Future Releases (v1.x)**

- [ ] Python bindings via PyO3
- [ ] GPU acceleration
- [ ] Cloud storage integration

## 🧪 Testing

```bash
# Run all tests
cargo test --all-features

# Run benchmarks
cargo bench --all-features
```

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/amazing-feature`
3. **Commit** your changes: `git commit -m 'Add amazing feature'`
4. **Push** to branch: `git push origin feature/amazing-feature`
5. **Open** a Pull Request

### Development Setup

```bash
# Clone repository
git clone https://github.com/elemeng/mrc.git
cd mrc

# Build with all features
cargo build --all-features

# Run tests
cargo test --all-features

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features --all-targets
```

## 📄 MIT License

```
MIT License

Copyright (c) 2024-2025 mrc contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## 🙏 Acknowledgments

- **CCP-EM** for the [MRC-2014 specification](https://www.ccpem.ac.uk/mrc-format/mrc2014/)
- **EMDB** for providing real-world test data
- **Cryo-EM community** for invaluable feedback
- **Rust community** for the amazing ecosystem

## 📞 Support & Community

- 🐛 **Issues**: [Report bugs](https://github.com/elemeng/mrc/issues)
- 📖 **Documentation**: [Full docs](https://docs.rs/mrc)
- 🏷️ **Releases**: [Changelog](https://github.com/elemeng/mrc/releases)

---

<div align="center">

**Made with ❤️ by the cryo-EM community for the scientific computing world**

*[SIMD-accelerated • Memory-mapped]*

</div>
