# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**mrc** is a zero-copy, zero-allocation MRC-2014 file format reader/writer for Rust, designed for cryo-electron microscopy and structural biology applications. It provides high-performance, memory-efficient handling of scientific image data.

## Architecture

The codebase follows a modular design with clear separation of concerns:

### Core Components

- **`src/lib.rs`**: Entry point with error types and re-exports
- **`src/header.rs`**: 1024-byte MRC header structure with validation and byte swapping
- **`src/mode.rs`**: Data type enumeration (Int8, Int16, Float32, etc.) with utility methods
- **`src/view.rs`**: Zero-copy data views (`MrcView`, `MrcViewMut`) for memory operations
- **`src/mrcfile.rs`**: File I/O operations (`MrcFile`, `MrcMmap`) for standard and memory-mapped access

### Feature Flags
- `std`: Standard library support (enabled by default)
- `mmap`: Memory-mapped I/O via `memmap2` (enabled by default)
- `file`: File operations (enabled by default)
- `f16`: Half-precision float support (requires nightly Rust)

## Development Commands

### Build & Test
```bash
# Build with all features
cargo build --all-features

# Run all tests
cargo test --all-features

# Run specific test module
cargo test --test mrcfile_test

# Run benchmarks
cargo bench

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features

# Build examples
cargo build --examples
```

### Examples
```bash
# Generate test MRC files
cargo run --example generate_mrc_files

# Validate MRC files
cargo run --example validate -- path/to/files/*.mrc

# Test with real MRC data
cargo run --example test_real_mrc
```

## Key Usage Patterns

### Reading MRC Files
```rust
use mrc::{MrcFile, MrcView};

// File-based access
let file = MrcFile::open("data.mrc")?;
let view = file.read_view()?;

// Memory-mapped for large files (mmap feature)
let mmap = MrcMmap::open("large.mrc")?;
let view = mmap.read_view()?;

// Zero-copy from memory
let view = MrcView::new(header, data)?;
```

### Creating MRC Files
```rust
use mrc::{Header, Mode, MrcFile};

let mut header = Header::new();
header.nx = 512;
header.ny = 512;
header.nz = 256;
header.mode = Mode::Float32 as i32;

let mut file = MrcFile::create("output.mrc", header)?;
file.write_data(&your_float_data)?;
```

### Data Access Patterns
```rust
// Type-safe access
match view.mode() {
    Some(Mode::Float32) => {
        let floats = view.view::<f32>()?;
        // Process as &[f32]
    }
    Some(Mode::Int16) => {
        let ints = view.view::<i16>()?;
        // Process as &[i16]
    }
    _ => return Err(Error::TypeMismatch),
}

// Mutable access
let mut view = MrcViewMut::new(header, &mut data)?;
{
    let floats = view.view_mut::<f32>()?;
    floats[0] = 42.0;
}
```

## Development Environment

- **Rust Toolchain**: Nightly required (see `rust-toolchain.toml`)
- **Components**: rustfmt, clippy
- **Target**: x86_64-unknown-linux-gnu

## Testing Strategy

- **Unit tests**: In-module tests for validation and edge cases
- **Integration tests**: `test/` directory for file I/O and real data testing
- **Benchmarks**: Performance testing for critical paths
- **Examples**: Real-world usage demonstrations

## Performance Considerations

- **Zero-copy**: Direct memory access with no intermediate buffers
- **Cache-friendly**: Aligned access patterns for SIMD operations
- **Memory-mapped**: Efficient large file handling via mmap feature
- **Branch prediction**: Optimized validation and mode checking

## Error Handling

All operations return `Result<T, mrc::Error>` with variants:
- `Io`: File I/O operations
- `InvalidHeader`: Malformed MRC header
- `InvalidMode`: Unsupported data type
- `InvalidDimensions`: Size mismatches
- `TypeMismatch`: Type casting failures
- `Mmap`: Memory mapping errors (mmap feature)