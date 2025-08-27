# 🧬 mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc)
[![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)
[![Build Status](https://img.shields.io/github/actions/workflow/status/your-org/mrc/ci.yml?branch=main)](https://github.com/elemeng/mrc/actions)

> **Zero-copy, zero-allocation, no_std-friendly MRC-2014 file format reader/writer for Rust**

A high-performance, memory-efficient library for reading and writing MRC (Medical Research Council) format files used in cryo-electron microscopy and structural biology. Designed for scientific computing with safety and performance as top priorities.

## ✨ Why mrc?

- **🚀 Zero-copy**: Direct memory mapping with no intermediate buffers
- **🦀 no_std**: Works in embedded environments and WebAssembly
- **⚡ Blazing fast**: Optimized for cache locality and branch prediction
- **🔒 100% safe**: No unsafe blocks in public API

**Note: This crate is currently under active development. While most features are functional, occasional bugs and API changes are possible. Contributions are welcome—please report issues and share your ideas!**

## 📦 Installation

```toml
[dependencies]
mrc = "0.1"

# For full features
mrc = { version = "0.1", features = ["std", "mmap", "file", "f16"] }
```

## 🚀 Quick Start

### 📖 Reading MRC Files

```rust
use mrc::MrcView;

// Read from memory
let data = std::fs::read("protein.mrc")?;
let view = MrcView::new(data)?;

// Get dimensions
let (nx, ny, nz) = view.dimensions();
println!("Volume: {}×{}×{} voxels", nx, ny, nz);

// Access data based on type
match view.mode() {
    Some(Mode::Float32) => {
        let floats = view.view::<f32>()?;
        println!("First pixel: {}", floats[0]);
    }
    Some(Mode::Int16) => {
        let ints = view.view::<i16>()?;
        println!("First pixel: {}", ints[0]);
    }
    _ => println!("Unsupported data type"),
}
```

### ✏️ Creating New Files

```rust
use mrc::{Header, Mode, MrcFile};

// Create header for 3D volume
let mut header = Header::new();
header.nx = 512;
header.ny = 512;
header.nz = 256;
header.mode = Mode::Float32 as i32;

// Set physical dimensions (Ångströms)
header.xlen = 256.0;
header.ylen = 256.0;
header.zlen = 128.0;

// Write to file
let mut file = MrcFile::create("output.mrc", header)?;
file.write_data(&your_data)?;
```

## 🤝 How to Contribute

**🐞 Issues & Bugs**  
Found a bug? [**Open an issue**](https://github.com/your-org/mrc/issues/new) — we’ll triage fast.

**💡 Feature Requests & Ideas**  
Tag your issue with `[Feature request]` — the community helps shape the roadmap.

**🦀 Pull Requests**  
Ready to code? See **Contributing** below. Fork → branch → PR. All skill levels welcome; CI and review keep quality high.

Built with ❤️ for every cryo-EM enthusiast.


## 🗺️ API Architecture

### Core Types Overview

| Type           | Purpose               | Example                       |
| -------------- | --------------------- | ----------------------------- |
| [`Header`]     | 1024-byte MRC header  | `let header = Header::new();` |
| [`Mode`]       | Data type enumeration | `Mode::Float32`               |
| [`MrcView`]    | Read-only data view   | `MrcView::new(data)?`         |
| [`MrcViewMut`] | Mutable data view     | `MrcViewMut::new(data)?`      |
| [`MrcFile`]    | File-backed access    | `MrcFile::open("file.mrc")?`  |
| [`MrcMmap`]    | Memory-mapped access  | `MrcMmap::open("large.mrc")?` |

## 📚 Detailed API Reference

### 🔧 Header Structure

The MRC header contains 56 fields (1024 bytes total) with complete metadata:

#### Creating Headers

```rust
use mrc::Header;

let mut header = Header::new();

// Basic dimensions
header.nx = 2048;        // X dimension
header.ny = 2048;        // Y dimension  
header.nz = 512;         // Z dimension

// Data type (see Mode enum)
header.mode = Mode::Float32 as i32;

// Physical dimensions in Ångströms
header.xlen = 204.8;     // Physical X length
header.ylen = 204.8;     // Physical Y length
header.zlen = 102.4;     // Physical Z length

// Cell angles for crystallography
header.alpha = 90.0;
header.beta = 90.0;
header.gamma = 90.0;

// Axis mapping (1=X, 2=Y, 3=Z)
header.mapc = 1;         // Fastest changing axis
header.mapr = 2;         // Second fastest axis
header.maps = 3;         // Slowest changing axis

// Data statistics
header.dmin = 0.0;       // Minimum data value
header.dmax = 1.0;       // Maximum data value
header.dmean = 0.5;      // Mean data value
header.rms = 0.1;        // RMS deviation
```

#### Header Fields Reference

| Field                | Type             | Description          | Range                    |
| -------------------- | ---------------- | -------------------- | ------------------------ |
| `nx, ny, nz`         | `i32`            | Image dimensions     | > 0                      |
| `mode`               | `i32`            | Data type            | 0-4, 6, 12, 101          |
| `mx, my, mz`         | `i32`            | Map dimensions       | Usually same as nx/ny/nz |
| `xlen, ylen, zlen`   | `f32`            | Cell dimensions (Å)  | > 0                      |
| `alpha, beta, gamma` | `f32`            | Cell angles (°)      | 0-180                    |
| `mapc, mapr, maps`   | `i32`            | Axis mapping         | 1, 2, 3                  |
| `amin, amax, amean`  | `f32`            | Origin coordinates   | -∞ to ∞                  |
| `ispg`               | `i32`            | Space group number   | 0-230                    |
| `nsymbt`             | `i32`            | Extended header size | ≥ 0                      |
| `extra`              | `[u8; 100]`      | Extra space          | -                        |
| `origin`             | `[i32; 3]`       | Origin coordinates   | -                        |
| `map`                | `[u8; 4]`        | Map string           | "MAP "                   |
| `machst`             | `[u8; 4]`        | Machine stamp        | -                        |
| `rms`                | `f32`            | RMS deviation        | ≥ 0                      |
| `nlabl`              | `i32`            | Number of labels     | 0-10                     |
| `label`              | `[[u8; 80]; 10]` | Text labels          | -                        |

### 📊 Data Type Support

| [`Mode`]         | Value | Rust Type | Bytes | Description           | Use Case           |
| ---------------- | ----- | --------- | ----- | --------------------- | ------------------ |
| `Int8`           | 0     | `i8`      | 1     | Signed 8-bit integer  | Binary masks       |
| `Int16`          | 1     | `i16`     | 2     | Signed 16-bit integer | Cryo-EM density    |
| `Float32`        | 2     | `f32`     | 4     | 32-bit float          | Standard density   |
| `Int16Complex`   | 3     | `i16`     | 2×2   | Complex 16-bit        | Phase data         |
| `Float32Complex` | 4     | `f32`     | 4×2   | Complex 32-bit        | Fourier transforms |
| `Uint16`         | 6     | `u16`     | 2     | Unsigned 16-bit       | Segmentation       |
| `Float16`        | 12    | `f16`[^1] | 2     | 16-bit float          | Memory efficiency  |

### 🔄 Data Access Patterns

#### Zero-Copy Read Access

```rust
use mrc::{MrcView, Error, Mode};

// From byte slice
let view = MrcView::new(header, data)?;

// Type-safe access
match view.mode() {
    Some(Mode::Float32) => {
        let floats = view.view::<f32>()?;
        // floats: &[f32] - zero-copy slice
        let sum: f32 = floats.iter().sum();
        println!("Total intensity: {}", sum);
    }
    Some(Mode::Int16) => {
        let ints = view.view::<i16>()?;
        // Process 16-bit integer data
        let max = ints.iter().max().unwrap_or(&0);
    }
    _ => return Err(Error::TypeMismatch),
}

// Raw byte access
let raw_bytes = view.data();           // &[u8]
let slice = view.slice_bytes(0..1024)?; // &[u8]
```

#### Mutable In-Place Editing

```rust
use mrc::{MrcViewMut, Header};

// Create mutable view
let mut view = MrcViewMut::new(header, &mut data)?;

// Modify data
{
    let mut floats = view.view_mut::<f32>()?;
    for val in floats.iter_mut() {
        *val = val.max(0.0);  // Remove negative values
    }
}

// Update header statistics
view.update_statistics()?;

// Modify header
let mut new_header = view.header().clone();
new_header.dmin = 0.0;
new_header.dmax = 1.0;
view.set_header(new_header)?;
```

#### File I/O Operations

```rust
use mrc::{MrcFile, MrcMmap, Mode};

// Standard file I/O
let file = MrcFile::open("data.mrc")?;
let view = file.view()?;

// Memory-mapped for large files (requires mmap feature)
#[cfg(feature = "mmap")]
let mmap = MrcMmap::open("large_volume.mrc")?;
#[cfg(feature = "mmap")]
let view = mmap.view()?;

// Write new file
let header = Header {
    nx: 512, ny: 512, nz: 256,
    mode: Mode::Float32 as i32,
    ..Header::new()
};

let mut file = MrcFile::create("output.mrc", header)?;
file.write_data(&your_float_data)?;
```

### 🧮 Advanced Operations

#### 3D Volume Processing

```rust
use mrc::MrcView;

struct Volume3D<'a> {
    view: MrcView<'a>,
    nx: usize,
    ny: usize,
    nz: usize,
}

impl<'a> Volume3D<'a> {
    fn new(view: MrcView<'a>) -> Result<Self, mrc::Error> {
        let (nx, ny, nz) = view.dimensions();
        Ok(Self { view, nx, ny, nz })
    }
    
    fn get_slice(&self, z: usize) -> Result<&[f32], mrc::Error> {
        if z >= self.nz {
            return Err(mrc::Error::InvalidDimensions);
        }
        
        let slice_size = self.nx * self.ny;
        let start = z * slice_size;
        let floats = self.view.view::<f32>()?;
        
        floats.get(start..start + slice_size)
            .ok_or(mrc::Error::InvalidDimensions)
    }
    
    fn get_voxel(&self, x: usize, y: usize, z: usize) -> Result<f32, mrc::Error> {
        let index = z * self.nx * self.ny + y * self.nx + x;
        let floats = self.view.view::<f32>()?;
        
        floats.get(index).copied()
            .ok_or(mrc::Error::InvalidDimensions)
    }
}
```

#### Batch Processing with Ray

```rust
use mrc::{MrcFile, Mode};
use rayon::prelude::*;

fn process_directory(dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    
    let entries = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "mrc"));
    
    entries.par_bridge().try_for_each(|entry| {
        let path = entry.path();
        let file = MrcFile::open(&path)?;
        let view = file.view()?;
        
        if let Some(Mode::Float32) = view.mode() {
            let data = view.view::<f32>()?;
            let stats = calculate_statistics(data);
            println!("{:?}: RMS={:.3}", path.file_name(), stats.rms);
        }
        
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    
    Ok(())
}

#[derive(Debug)]
struct Statistics {
    min: f32,
    max: f32,
    mean: f32,
    rms: f32,
}

fn calculate_statistics(data: &[f32]) -> Statistics {
    let min = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let mean = data.iter().sum::<f32>() / data.len() as f32;
    let rms = (data.iter().map(|&x| x * x).sum::<f32>() / data.len() as f32).sqrt();
    
    Statistics { min, max, mean, rms }
}
```

## 🎯 Feature Flags

| Feature | Description              | Default | no_std Compatible | Example                                 |
| ------- | ------------------------ | ------- | ----------------- | --------------------------------------- |
| `std`   | Standard library support | ✅       | ❌                | File I/O, Error trait                   |
| `mmap`  | Memory-mapped I/O        | ✅       | ❌                | Large file processing                   |
| `file`  | File operations          | ✅       | ❌                | `MrcFile::open()`                       |
| `f16`   | Half-precision support   | ✅       | ❌                | `view::<f16>()` with IEEE 754-2008 half |

### no_std Usage

**For embedded systems, WebAssembly, and RTOS environments:**

```toml
[dependencies]
mrc = { version = "0.1", default-features = false }
```

```rust
use mrc::{Header, MrcView, Mode};

// Pure no_std usage - works in embedded/WebAssembly
let mut header = Header::new();
header.nx = 256;
header.ny = 256;
header.nz = 100;
header.mode = Mode::Float32 as i32;

// Your byte buffer (from flash, network, etc.)
let data = &[your_byte_data];
let view = MrcView::new(header, data)?;
let floats = view.view::<f32>()?;

// Process without any file system dependencies
let sum: f32 = floats.iter().sum();
```

### no_std Compatible APIs

| API | Available in no_std | Description |
| --- | ------------------- | ----------- |
| `Header` | ✅ | 1024-byte MRC header structure |
| `Mode` | ✅ | Data type enumeration |
| `MrcView` | ✅ | Zero-copy read-only data access |
| `MrcViewMut` | ✅ | Zero-copy mutable data access |
| `Error` | ✅ | Comprehensive error handling |
| `MrcFile` | ❌ | Requires file system (std) |
| `MrcMmap` | ❌ | Requires memory mapping (std) |
| `f16` support | ❌ | Requires half crate (std) | 

## 🛣️ Development Roadmap

### ✅ **Current Release (v0.1.x): Core ability**
- [x] Complete MRC-2014 format support
- [x] Zero-copy memory access
- [x] All data types (modes 0-4, 6, 12, 101) including mode **101** (4-bit data packed two per byte)
- [x] Header manipulation
- [x] File I/O operations
- [x] Memory-mapped I/O
- [x] Comprehensive documentation

### 🚧 **Next Release (v0.2.x): Rich features**
- [x] **Validation utilities** for data integrity
- [ ] **Streaming API** for large datasets
- [ ] **Compression support** (gzip, zstd)
- [ ] **Statistics functions** (histogram, moments)
- [ ] **Python bindings** via PyO3
- [ ] **Extended header** for "CCP4, SERI, AGAR, FEI1, FEI2, HDF5"

### 🚀 **Future Releases (v1): Super features**
- [ ] **implement 100% features of the official python lib mrcfile** 
- [ ] **Image processing** (filters, transforms)
- [ ] **GPU acceleration** support
- [ ] **WebAssembly** target
- [ ] **Cloud storage** integration
- [ ] **Parallel processing** utilities
- [ ] **Visualization helpers**

**Awayls using only features that you need to minimize sizes of the package**

## 📊 Performance Benchmarks

### 💾 Memory Efficiency
- **Header**: Fixed 1024 bytes (no heap allocation)
- **Data views**: Zero-copy slices
- **Extended headers**: Lazy loaded
- **File handles**: Minimal overhead

### ⚡ Optimization Tips
```rust
// Use memory mapping for large files
#[cfg(feature = "mmap")]
let view = MrcMmap::open("large.mrc")?.view()?;

// Cache data size calculations
let data_size = view.header().data_size();

// Use aligned access for SIMD
let aligned = view.data_aligned::<f32>()?;
```

## 🧪 Testing Examples

### Unit Tests
```bash
# Run all tests
cargo test --all-features

# Run specific test
cargo test test_header_validation

# Run with coverage
# install tarpaulin if not existed: cargo install cargo-tarpaulin
cargo tarpaulin --all-features
```

### Integration Tests
```bash
# Test with real MRC files
cargo test --test real_mrc_files

# Benchmark performance
cargo bench
```

### Example Programs
```bash
# Generate test MRC files
cargo run --example generate_mrc_files

# Validate MRC files
cargo run --example validate -- data/*.mrc
```

## 🤝 Contributing guide

We welcome contributions! Here's how to get started:

### 📋 Contribution Guide
1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/amazing-feature`
3. **Commit** your changes: `git commit -m 'Add amazing feature'`
4. **Push** to branch: `git push origin feature/amazing-feature`
5. **Open** a Pull Request

### 🏗️ Development Setup
```bash
# Clone repository
git clone https://github.com/your-org/mrc.git
cd mrc

# Install dependencies
cargo build --all-features

# Run tests
cargo test --all-features

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features
```

### 📄 Code Standards
- **100% safe Rust** (no unsafe blocks)
- **Comprehensive tests** for all functionality
- **Documentation** for all public APIs
- **Performance benchmarks** for critical paths

## 📄 MIT License

```
MIT License

Copyright (c) 2024 mrc contributors

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

- 💁 **Helps**: Directly open an issue to ask for help is wellcome. Add a **Help** tag.
- 🐛 **Issues**: [Report bugs](https://github.com/elemeng/mrc/issues)
- 📖 **Documentation**: [Full docs](https://docs.rs/mrc)
- 🏷️ **Releases**: [Changelog](https://github.com/elemeng/mrc/releases)

---

<div align="center">

**Made with ❤️ by the cryo-EM community for the scientific computing world**

*[Zero-copy • Zero-allocation • 100% safe Rust]*

</div> 
