# 🧬 mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc) [![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)

> **Zero-copy, zero-allocation, no_std-friendly MRC-2014 file format reader/writer for Rust**

A high-performance, memory-efficient library for reading and writing MRC (Medical Research Council) format files used in cryo-electron microscopy and structural biology. Designed for scientific computing with safety and performance as top priorities.

## ✨ Why mrc?

- **🚀 Zero-copy**: Direct memory mapping with no intermediate buffers
- **🦀 no_std**: Works in embedded environments and WebAssembly
- **⚡ SIMD-accelerated**: AVX2/NEON accelerated type conversions
- **🔄 Type conversion**: Automatic conversion between voxel types
- **🔒 100% safe**: No unsafe blocks in public API

**Note: This crate is currently under active development. While most features are functional, occasional bugs and API changes are possible. Contributions are welcome—please report issues and share your ideas!**

## 📦 Installation

```toml
[dependencies]
mrc = "0.2"

# For all features
mrc = { version = "0.2", features = ["std", "mmap", "f16", "simd", "parallel"] }
```

## 🚀 Quick Start

### Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌────────────────┐
│   File System   │────▶│  Header Parsing  │────▶│  Iterator API  │
│   (.mrc file)   │     │   (1024 bytes)   │     │  (Zero-copy)   │
└─────────────────┘     └──────────────────┘     └────────────────┘
         │                       │                       │
   ┌─────────────┐          ┌────────┐              ┌─────────┐
   │ Reader      │          │ Header │              │ VoxelBlock
   │ MmapReader  │          │        │              │         │
   │ Writer      │          └────────┘              └─────────┘
   │ MmapWriter  │
   └─────────────┘
```

### MRC File Structure

```text
| 1024 bytes | NSYMBT bytes | data_size bytes |
|  header    | ext header   | voxel data      |
```

### 📖 Reading MRC Files

```rust
use mrc::Reader;

fn main() -> Result<(), mrc::Error> {
    // Open an MRC file - header is parsed automatically
    let reader = Reader::open("protein.mrc")?;

    // Get volume dimensions
    let shape = reader.shape();
    println!("Volume: {}×{}×{} voxels", shape.nx, shape.ny, shape.nz);

    // Iterate over slices - zero-copy when file type matches
    for slice in reader.slices::<f32>() {
        let block = slice?;  // VoxelBlock<f32>
        println!("Slice {}: {} voxels", block.offset[2], block.len());
    }

    // Or read with automatic type conversion
    // Read Int16 file as Float32 (uses SIMD when available)
    for slice in reader.slices_converted::<i16, f32>() {
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
        );
        writer.write_block(&block)?;
    }

    // Finalize writes the header with correct statistics
    writer.finalize()?;
    Ok(())
}
```

## ⚠️ Migrating from v0.1

v0.2 is a complete architectural redesign. Key API changes:

| v0.1 | v0.2 |
|------|------|
| `MrcView::new(data)` | `Reader::open(path)` |
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
let reader = Reader::open("file.mrc")?;
for slice in reader.slices::<f32>() {
    let block = slice?;
    // process block.data
}
```

**New in v0.2:** SIMD acceleration, parallel encoding, type conversion iterators, `MmapReader`.

## 🗺️ API Overview

### Core Types

| Type | Purpose | Example |
|------|---------|---------|
| [`Reader`] | Read MRC files | `Reader::open("file.mrc")?` |
| [`MmapReader`] | Memory-mapped reading | `MmapReader::open("large.mrc")?` |
| [`Writer`] | Write MRC files | `create("out.mrc").shape([64,64,64]).mode::<f32>().finish()?` |
| [`MmapWriter`] | Memory-mapped writing | `MmapWriter::create("out.mrc", header)?` |
| [`WriterBuilder`] | Configure new files | `create(path).shape(dims).mode::<T>()` |
| [`Header`] | 1024-byte MRC header | `Header::new()` |
| [`Mode`] | Data type enumeration | `Mode::Float32` |
| [`VoxelBlock<T>`] | Chunk of voxel data | `VoxelBlock::new(offset, shape, data)` |
| [`VolumeShape`] | Volume dimensions | `VolumeShape::new(nx, ny, nz)` |

### Iterator API

The library provides an iterator-centric API for efficient processing:

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

// Iterate over arbitrary chunks
for chunk in reader.blocks::<f32>([64, 64, 64]) {
    let block = chunk?;
    // Process 64³ chunk
}
```

### Type Conversion

Automatic type conversion is supported via the `Convert` trait:

```rust
// Read file as one type, convert to another
for slice in reader.slices_converted::<i16, f32>() {
    let block = slice?;
    // i16 data automatically converted to f32
}

// Write with type conversion
let mut writer = create("output.mrc")
    .shape([256, 256, 128])
    .mode::<i16>()  // File stores i16
    .finish()?;

let f32_data: VoxelBlock<f32> = ...;
writer.write_converted::<f32, i16>(&f32_data)?;  // Converts f32 -> i16
```

### Memory-Mapped I/O

For large files (>1GB), memory-mapped I/O lets the OS handle paging:

```rust
use mrc::MmapReader;

let reader = MmapReader::open("large_volume.mrc")?;

// Same iterator API as Reader
for slice in reader.slices::<f32>() {
    let block = slice?;
    // OS automatically pages data in/out
}

// Direct byte access for zero-copy scenarios
if reader.can_zero_copy::<f32>() {
    let bytes = reader.data_bytes();  // &[u8] backed by mmap
}
```

| Use | When |
|-----|------|
| `Reader` | Small files, `no_std` compatibility, simple sequential access |
| `MmapReader` | Large files, memory-constrained environments, random access |

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
| `Packed4Bit` | 101 | [`Packed4Bit`] | 0.5 | Packed 4-bit | Compression |

[^1]: Requires `f16` feature and nightly Rust compiler.

## ⚡ Performance Features

### SIMD Acceleration

The library automatically uses SIMD instructions (AVX2 on x86_64, NEON on AArch64) for type conversions:

```rust
// These conversions use SIMD when available:
// i8 → f32, i16 → f32, u16 → f32, u8 → f32

// Access SIMD functions directly
#[cfg(feature = "simd")]
{
    let f32_data = mrc::convert_i16_slice_to_f32(&i16_data);
}
```

### Zero-Copy Reading

When the file's native type matches your target type and endianness:

```rust
let reader = Reader::open("data.mrc")?;

// Check if zero-copy is possible
if reader.can_zero_copy::<f32>() {
    println!("Using zero-copy fast path!");
}

// The iterator will use zero-copy automatically when possible
for slice in reader.slices::<f32>() {
    // No allocation or conversion happening here
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

## 🎯 Feature Flags

| Feature | Description | Default | no_std Compatible |
|---------|-------------|---------|-------------------|
| `std` | Standard library support | ✅ | ❌ |
| `mmap` | Memory-mapped I/O | ✅ | ❌ |
| `f16` | Half-precision support | ✅ | ❌ |
| `simd` | SIMD acceleration | ✅ | ❌ |
| `parallel` | Parallel encoding | ✅ | ❌ |

### no_std Usage

For embedded systems, WebAssembly, and other constrained environments:

```toml
[dependencies]
mrc = { version = "0.2", default-features = false }
```

```rust
#![no_std]
use mrc::{Header, Mode};

// Create a header (works in no_std)
let mut header = Header::new();
header.nx = 256;
header.ny = 256;
header.nz = 100;
header.mode = Mode::Float32 as i32;
```

### no_std Compatible APIs

| API | Available in no_std | Description |
|-----|---------------------|-------------|
| `Header` | ✅ | 1024-byte MRC header structure |
| `Mode` | ✅ | Data type enumeration |
| `VoxelBlock` | ✅ | Voxel data container |
| `VolumeShape` | ✅ | Volume geometry |
| `Convert` trait | ✅ | Type conversion |
| `Reader` | ❌ | Requires file system |
| `Writer` | ❌ | Requires file system |
| SIMD functions | ❌ | Requires std |

## 🔧 Header Structure

The MRC header contains 56 fields (1024 bytes total):

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
header.set_exttyp_str("FEI1")?;
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

## 🛣️ Development Roadmap

### ✅ **Current Release (v0.2.x): Core + SIMD**

- [x] Complete MRC-2014 format support
- [x] Iterator-centric API (slices, slabs, blocks)
- [x] Type conversion pipeline
- [x] SIMD acceleration (AVX2, NEON)
- [x] Zero-copy fast paths
- [x] Parallel encoding
- [x] Memory-mapped I/O (`MmapReader`, `MmapWriter`)
- [x] All data types (modes 0-4, 6, 12, 101)
- [x] no_std support

### 🚧 **Next Release (v0.3.x): Extended Features**

- [ ] Extended header parsing (CCP4, FEI1, FEI2, etc.)
- [ ] Statistics functions (histogram, moments)
- [ ] Validation utilities
- [ ] Streaming API for very large datasets

### 🚀 **Future Releases (v1.x)**

- [ ] Python bindings via PyO3
- [ ] GPU acceleration
- [ ] Compression support (gzip, zstd)
- [ ] Cloud storage integration

## 🧪 Testing

```bash
# Run all tests
cargo test --all-features

# Run benchmarks
cargo bench --all-features

# Check no_std build
cargo check --no-default-features
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
cargo clippy --all-features
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

*[Zero-copy • SIMD-accelerated • 100% safe Rust]*

</div>
