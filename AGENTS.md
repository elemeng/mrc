# AGENTS.md - AI Coding Agent Guide for mrc

This file provides essential information for AI coding agents working with the `mrc` Rust project.

## Project Overview

**mrc** is a zero-copy, zero-allocation MRC-2014 file format reader/writer for Rust. It provides high-performance, memory-efficient access to MRC (Medical Research Council) files used in cryo-electron microscopy and structural biology.

- **Version**: 0.2.0
- **Rust Edition**: 2024
- **Minimum Rust Version**: 1.85
- **License**: MIT
- **Repository**: https://github.com/elemeng/mrc

## Key Features

- **Zero-copy memory access**: Direct slice views into data without allocation
- **no_std compatible**: Works in embedded environments and WebAssembly
- **Type-safe**: Enum-based mode system with compile-time checks
- **Memory safety**: Lifetime-based borrowing prevents use-after-free
- **Endianness handling**: Automatic detection and conversion of file endianness

## Architecture

### Component Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   File System   │───▶│  Header Parsing  │───▶│   View Layer    │
│   (.mrc file)   │    │   (1024 bytes)   │    │ (Zero-copy)     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
    ┌────────┐              ┌────────┐              ┌────────┐
    │ MrcFile│              │ Header │              │ MrcView│
    │MrcMmap │              │        │              │        │
    └────────┘              └────────┘              └────────┘
```

### File Layout

```text
| 1024 bytes | NSYMBT bytes | data_size bytes |
|  header    | ext header   | voxel data      |
```

### Module Structure

| File | Purpose | Key Types |
|------|---------|-----------|
| `src/lib.rs` | Core library, traits, error types | `FileEndian`, `DataBlock`, `DecodeFromFile`, `EncodeToFile`, `Error` |
| `src/header.rs` | 1024-byte MRC header | `Header` |
| `src/mode.rs` | Data type enumeration | `Mode` |
| `src/view.rs` | Zero-copy data views | `MrcView`, `MrcViewMut` |
| `src/mrcfile.rs` | File I/O operations | `MrcFile`, `MrcMmap` |

## Technology Stack

- **Language**: Rust (Edition 2024, MSRV 1.85)
- **Key Dependencies**:
  - `bytemuck` (1.15): Safe type casting
  - `memmap2` (0.9): Memory-mapped file I/O (optional)
  - `thiserror` (2.0.16): Error handling
  - Nightly Rust with `f16` feature enabled

## Feature Flags

| Feature | Description | Default | no_std Compatible |
|---------|-------------|---------|-------------------|
| `std` | Standard library support | ✅ | ❌ |
| `mmap` | Memory-mapped I/O via memmap2 | ✅ | ❌ |
| `file` | File operations | ✅ | ❌ |
| `f16` | Half-precision float support | ✅ | ❌ |

### no_std Usage

```toml
[dependencies]
mrc = { version = "0.2", default-features = false }
```

## Build Commands

```bash
# Build with all features
cargo build --all-features

# Build for no_std environments
cargo build --no-default-features

# Build with specific features
cargo build --features "std,file"
```

## Test Commands

```bash
# Run all tests
cargo test --all-features

# Run unit tests only
cargo test --lib --all-features

# Run integration tests
cargo test --test tests --all-features
cargo test --test mrcfile_test --all-features

# Run specific test modules
cargo test header_tests --all-features
cargo test view_tests --all-features

# Run with coverage (requires cargo-tarpaulin)
cargo install cargo-tarpaulin
cargo tarpaulin --all-features
```

## Benchmark Commands

```bash
# Run all benchmarks
cargo bench

# Run specific benchmarks
cargo bench --bench benchmark
cargo bench --bench performance
cargo bench --bench encode_decode_bench
```

## Example Commands

```bash
# Generate test MRC files
cargo run --example generate_mrc_files

# Validate MRC files
cargo run --example validate -- path/to/file.mrc

# Test all features
cargo run --example test_all_features

# Test with real MRC files
cargo run --example test_real_mrc
```

## Code Style Guidelines

### General Principles

1. **100% safe Rust**: No unsafe blocks in public API
2. **Zero-copy design**: Minimize allocations, use slices and references
3. **Explicit error handling**: Use `Result<T, Error>` for all fallible operations
4. **Type safety**: Leverage Rust's type system to prevent invalid states

### Naming Conventions

- Types: `PascalCase` (e.g., `MrcView`, `DataBlock`)
- Functions/Methods: `snake_case` (e.g., `from_parts`, `data_size`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `SIZE`)
- Generic parameters: Single uppercase letters (e.g., `'a`, `T`)

### Documentation Style

- Use `//!` for module-level documentation
- Use `///` for item-level documentation
- Include examples in doc comments where applicable
- Document panics, errors, and safety requirements explicitly

### Error Handling Patterns

```rust
// Use the crate's Error type
use crate::Error;

// Return Result for fallible operations
pub fn from_parts(header: Header, ...) -> Result<Self, Error> {
    if !header.validate() {
        return Err(Error::InvalidHeader);
    }
    // ...
}
```

## Supported MRC Modes

| Mode | Value | Rust Type | Bytes | Description |
|------|-------|-----------|-------|-------------|
| `Int8` | 0 | `i8` | 1 | Signed 8-bit integer |
| `Int16` | 1 | `i16` | 2 | Signed 16-bit integer |
| `Float32` | 2 | `f32` | 4 | 32-bit float (default) |
| `Int16Complex` | 3 | `Int16Complex` | 4 | Complex 16-bit |
| `Float32Complex` | 4 | `Float32Complex` | 8 | Complex 32-bit |
| `Uint16` | 6 | `u16` | 2 | Unsigned 16-bit integer |
| `Float16` | 12 | `f16` | 2 | 16-bit float (requires `f16` feature, nightly only) |
| `Packed4Bit` | 101 | `Packed4Bit` | 1 | 4-bit packed data |

## Testing Strategy

### Test Organization

- **Unit tests**: Embedded in source files under `#[cfg(test)]`
- **Integration tests**: `test/tests.rs` (header, mode, view tests)
- **File I/O tests**: `test/mrcfile_test.rs` (requires `std` feature)
- **Benchmarks**: `test/benchmark.rs`, `test/performance.rs`

### Test Patterns

```rust
#[cfg(test)]
mod tests {
    use crate::{Header, Mode, MrcView};
    use alloc::vec;

    #[test]
    fn test_example() {
        let mut header = Header::new();
        header.nx = 64;
        header.ny = 64;
        header.nz = 64;
        header.mode = 2; // Float32
        
        let data_size = header.data_size();
        let data = vec![0u8; data_size];
        let view = MrcView::from_parts(header, &[], &data).unwrap();
        
        assert_eq!(view.dimensions(), (64, 64, 64));
    }
}
```

## Development Workflow

### Before Committing

```bash
# Check formatting
cargo fmt --check

# Run clippy lints
cargo clippy --all-features -- -D warnings

# Run all tests
cargo test --all-features

# Generate documentation
cargo doc --all-features --no-deps
```

### Adding New Features

1. Add implementation to appropriate module
2. Add unit tests in the same file under `#[cfg(test)]`
3. Add integration tests in `test/tests.rs` if needed
4. Update documentation and examples
5. Run full test suite

## Common Tasks

### Creating a New MRC File

```rust
use mrc::{Header, Mode, MrcFile};

let mut header = Header::new();
header.nx = 512;
header.ny = 512;
header.nz = 256;
header.mode = Mode::Float32 as i32;
header.xlen = 256.0;
header.ylen = 256.0;
header.zlen = 128.0;

let mut file = MrcFile::create("output.mrc", header)?;
file.write_data(&your_data)?;
```

### Reading an MRC File

```rust
use mrc::{MrcFile, Mode};

let file = MrcFile::open("data.mrc")?;
let view = file.read_view()?;

match view.mode() {
    Some(Mode::Float32) => {
        let floats = view.data.to_vec_f32()?;
        // Process data
    }
    _ => {}
}
```

### Using Memory-Mapped I/O

```rust
use mrc::MrcMmap;

let mmap = MrcMmap::open("large_volume.mrc")?;
let view = mmap.read_view()?;
// Access data without loading entire file into memory
```

## Security Considerations

1. **Input Validation**: Always validate headers before processing (`header.validate()`)
2. **Buffer Sizes**: Ensure data buffers match expected sizes from header
3. **Endianness**: File endianness is automatically detected and handled
4. **No Unsafe Code**: Public API contains no unsafe blocks

## Resources

- **MRC-2014 Specification**: https://www.ccpem.ac.uk/mrc-format/mrc2014/
- **Crate Documentation**: https://docs.rs/mrc
- **Repository**: https://github.com/elemeng/mrc

## Notes for AI Agents

- This project prioritizes **safety and correctness** over performance optimizations
- Always use `Header::decode_from_bytes()` and `Header::encode_to_bytes()` for header I/O
- The `DataBlock` and `DataBlockMut` types handle endianness conversion transparently
- Extended headers are treated as opaque byte sequences (no interpretation)
- When in doubt, follow existing patterns in the codebase
