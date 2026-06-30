# Agent Guide for `mrc`

This file contains project-specific context for AI coding agents working on the `mrc` crate. The reader is assumed to know nothing about the project.

## Project Overview

`mrc` is a Rust library crate that reads and writes MRC-2014 files, a binary format used in cryo-electron microscopy (cryo-EM) and structural biology. The crate prioritizes zero-copy access, type-safe I/O, and SIMD acceleration for common data conversion paths.

- **Repository**: https://github.com/elemeng/mrc
- **Crate**: https://crates.io/crates/mrc
- **Docs**: https://docs.rs/mrc
- **License**: MIT
- **Version**: 0.2.3

A reference Python implementation (`mrcfile/`) is vendored in the repo for specification comparison, but it is **not** part of the Rust build and is gitignored in releases.

## Technology Stack

- **Language**: Rust, Edition 2024, MSRV 1.85
- **Build Tool**: Cargo
- **CI**: GitHub Actions (`.github/workflows/rust.yml`) — builds and tests on `ubuntu-latest` for pushes/PRs to `main`
- **Error Handling**: `thiserror` (no-std compatible)
- **Optional Dependencies**:
  - `memmap2` — memory-mapped I/O (`mmap` feature)
  - `rayon` — parallel encoding (`parallel` feature)
  - `flate2` — gzip compression (`gzip` feature)
  - `bzip2` — bzip2 compression (`bzip2` feature)

## Build and Test Commands

```bash
# Build with all features (recommended for development)
cargo build --all-features

# Run all tests (unit + doc tests)
cargo test --all-features

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features

# Build release binaries
cargo build --release --bin mrc-validate
cargo build --release --bin mrc-header
cargo build --release --bin mrc-invert
```

There are **no integration test directories** (`tests/` or `benches/`). All tests are inline `#[cfg(test)]` modules inside source files.

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `mmap` | Memory-mapped readers/writers via `memmap2` | ✅ |
| `f16` | Half-precision `f16` support via the `half` crate | ✅ |
| `simd` | AVX2/NEON accelerated conversions (`i16→f32`, etc.) | ✅ |
| `parallel` | Parallel encoding via `rayon` | ✅ |
| `gzip` | Gzip-compressed MRC I/O | ✅ |
| `bzip2` | Bzip2-compressed MRC I/O | ❌ |

The `f16` feature enables half-precision float support via the `half` crate and works on stable Rust.

## Code Organization

```
src/
├── lib.rs              # Public API re-exports and convenience functions (open, create)
├── error.rs            # Top-level `Error` and `HeaderValidationError` enums (thiserror)
├── mode.rs             # `Mode` enum, `Voxel` trait, complex types, Packed4Bit
├── header.rs           # `Header` struct (1024-byte MRC-2014 header), `HeaderBuilder`
├── fei.rs              # FEI1/FEI2 extended header parsers
├── iter.rs             # `RegionIter`, steppers (`SliceStepper`, `SlabStepper`, `TileStepper`)
├── engine/
│   ├── mod.rs
│   ├── block.rs        # `VolumeShape`, `VoxelBlock<T>`
│   ├── codec.rs        # `EndianCodec` trait, `decode_slice`, `encode_slice`
│   ├── convert.rs      # Type conversion utilities (i16→f32, etc.)
│   ├── endian.rs       # `FileEndian` enum and detection
│   ├── simd.rs         # AVX2/NEON SIMD kernels (unsafe)
│   └── stats.rs        # Statistics computation for header validation
├── io/
│   ├── mod.rs
│   ├── reader.rs       # `CompressionType` and `detect_compression` helpers
│   ├── reader_common.rs# Shared `VoxelSource` trait and helper functions
│   ├── buffered.rs     # In-memory `Reader`
│   ├── mmap_reader.rs  # `MmapReader` (zero-copy, requires `mmap`)
│   ├── writer.rs       # `Writer`, `WriterBuilder`, `MmapWriter`, compressed writers
│   ├── gzip.rs         # `GzipWriter`, `GzipCompressor` (requires `gzip`)
│   └── bzip2.rs        # `Bzip2Writer`, `Bzip2Compressor` (requires `bzip2`)
└── bin/
    ├── mrc-validate.rs # CLI validation tool (`cargo run --bin mrc-validate`)
    ├── mrc-header.rs   # CLI header inspector (`cargo run --bin mrc-header`)
    └── mrc-invert.rs   # CLI contrast inverter (`cargo run --bin mrc-invert`)
```

### Module Philosophy

- `engine/` contains low-level, format-agnostic encoding/decoding primitives.
- `io/` contains user-facing I/O strategies (buffered, mmap, compressed).
- `iter/` provides lazy iterators that work over any `VoxelSource` implementor.
- The crate uses **sealed traits** (`VoxelSource`) to keep internal abstractions internal.

### API Surface Discipline

The top-level `lib.rs` is the *only* public entry point. Internal modules (`engine/`,
`io/`, `iter/`, `fei/`) are marked `mod` (private) or, when their items must be
re-exported, are `pub mod` but with `#[doc(hidden)]` on internal plumbing:

| Visibility | Items |
|------------|-------|
| **Public (in lib.rs)** | `open`, `create`, `Reader`, `WriterBuilder`, `Writer`, `Header`, `HeaderBuilder`, `Mode`, `Voxel`, `VoxelBlock`, `VolumeShape`, `RegionIter`, `FileEndian`, `Error`, `MmapReader`, `MmapWriter`, `GzipWriter`, `Bzip2Writer`, FEI types |
| **`#[doc(hidden)]`** | `VoxelSource`, `ReaderCore`, `EndianCodec`, `Stepper`, `Compressor`, `MachstInfo`, `CompressionType`, `detect_compression`, `GzipCompressor`, `Bzip2Compressor` |
| **`pub(crate)` only** | `validate_block_bounds`, `gather_block_bytes`, `decode_block`, `decode_native_endian`, `parse_header`, `DecompressedMrc`, `open_compressed`, `encode_block_to_buf` |

## Development Conventions

### Code Style

- **Language**: All comments, docs, and identifiers are in English.
- **Formatting**: Standard `rustfmt`. No custom `rustfmt.toml`.
- **Clippy**: The crate enforces `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` in `lib.rs`. Production code must not use `.unwrap()` or `.expect()`.
- **Inlining**: Small accessor methods and hot-path conversion functions are marked `#[inline]`.
- **Documentation**: Heavy use of `//!` module docs and `///` item docs. Doc-tests are present and run with `cargo test`.

### Error Handling

- Fallible functions return `Result<T, Error>`.
- `Error` is a central enum using `thiserror` for `#[from]` conversions.
- `ModeMismatch` and `TypeMismatch` errors are preferred over silent data corruption.
- Bounds checking on `VoxelBlock` shapes is mandatory.

### Type Safety

- The `Voxel` trait connects Rust types to MRC modes at compile time.
- Generic read/write APIs require `T: Voxel`, preventing runtime mode mismatches.
- Built-in conversion conveniences: `slices_f32()`, `slabs_f32()`, `slices_u8()`, `slices_mode0()`, `slabs_mode0()`, `write_u8_block()`, and `write_f16_from_f32()`. All other type conversion is the caller's responsibility.

## Testing Strategy

- **Unit Tests**: 50+ tests in inline `mod tests` blocks inside source files (`header.rs`, `engine/simd.rs`, `engine/convert.rs`, `engine/endian.rs`, `engine/stats.rs`, `io/reader.rs`).
- **Doc Tests**: Multiple doc-tests in `lib.rs`, `io/buffered.rs`, `io/writer.rs`, `io/mmap_reader.rs`, `header.rs`.
- **No External Fixtures**: Tests generate temporary MRC files programmatically (using `tempfile` in dev-dependencies) rather than checking large binary files into git.
- **Coverage Gaps**: There is no dedicated benchmark suite (criterion is in dev-dependencies but no `benches/` directory exists).

## Safety and Unsafe Code

The crate contains a small amount of `unsafe` Rust, all justified by performance:

1. **SIMD Kernels** (`engine/simd.rs`): AVX2 and NEON intrinsics for `i8→f32`, `i16→f32`, `u16→f32`. Runtime feature detection gates these.
2. **Memory Mapping** (`io/mmap_reader.rs`, `io/writer.rs`): `memmap2::MmapOptions::new().map()` requires `unsafe`.
3. **Fast-path memcpy** (`engine/codec.rs`): `core::ptr::copy_nonoverlapping` is used for native-endian decode/encode to avoid per-element branching.
4. **`Vec::set_len`** (`engine/codec.rs`): Used after `Vec::with_capacity` when all elements are guaranteed to be overwritten immediately.

**Agent Guidance**: When modifying unsafe code, ensure:
- Runtime feature detection for SIMD (do not assume AVX2/NEON is available).
- Alignment and size invariants are documented with `// SAFETY:` comments.
- No undefined behavior is introduced through out-of-bounds raw pointer access.

## Known Issues and Technical Debt

A detailed code review exists in `review.md` at the repository root. Items agents should be aware of:

1. **`gather_block_bytes` fast-path assumes contiguous XY slabs**: For full-row slabs (`ox == 0 && sx == nx && oy == 0 && sy == ny`) a contiguous copy is used. Sub-XY blocks correctly use row-by-row scatter/gather.
2. ~~**`decode_slice` panics on misaligned byte count** (Medium): If `bytes.len()` is not a multiple of `T::BYTE_SIZE`, `decode_slice` panics rather than returning a `Result`.~~ **Fixed**: Now returns `Result<Vec<T>, Error>`, propagating the error through `compute_stats` and `decode_block`.
3. ~~**`encode_slice` asserts length match** (Medium): `assert_eq!` previously used instead of returning a `Result`.~~ **Fixed**: Now returns `Result<(), Error>`.
4. ~~**Duplicated writer logic**: The scatter-path write loop (`write_block` for sub-XY blocks) was copy-pasted across `Writer`, `MmapWriter`, and `CompressedWriter`.~~ **Fixed**: Extracted shared `encode_block_to_buf` helper in `reader_common.rs`. MmapWriter and CompressedWriter now delegate to it.
5. ~~**`slices_u8` return type triggers clippy `type_complexity`**: The `Result<Box<dyn Iterator<...>>>` return type...~~ **Fixed**: `slices_u8` now returns `VoxelIter<'_, u8>` directly (matching `slices_f32`), with the mode check baked into the returned iterator.
6. ~~**`VoxelBlock::new` panics on shape mismatch** (Medium): The primary public constructor panicked on mismatched data length; `try_new` was the `Result`-based alternative.~~ **Fixed**: `new` now delegates to `try_new` and returns `Result<VoxelBlock<T>, Error>`. All call sites updated with `?` or `.unwrap()`.
7. **`TileStepper` edge-case confidence**: The tile-stepping logic is hard to visually verify for exact boundary conditions.
8. **`MmapReader::data_bytes()` silently truncates on undersized files in permissive mode**: When the file is smaller than the header claims, the method returns whatever bytes are available instead of signalling an error. In strict mode the file size is validated on open.

Agents should read `review.md` before making large refactors to I/O or header code.

## Deployment and Release

- The crate is published to **crates.io**.
- `cargo build --release` produces optimized artifacts.
- The `mrc-validate` binary can be distributed as a standalone validation tool.
- CI only runs on Ubuntu; there is no cross-platform or Windows/macOS testing in CI.

## Security Considerations

- **File Size Validation**: Readers validate that file size matches header-declared data size (with a `FileSizeMismatch` error) unless opened in permissive mode.
- **Memory Mapping**: `MmapReader` maps files read-only. `MmapWriter` maps read-write and can mutate files in place.
- **Compression**: Gzip/Bzip2 readers decompress the entire file into memory on open (they do not stream). This makes them susceptible to decompression bombs / zip bombs. Do not use these on untrusted input without size limits.
- **No `unsafe` in public API**: All `unsafe` is internal; the public API is 100% safe Rust.
- **Integer Overflow**: The codebase uses `checked_mul` and `checked_add` for size calculations in several places (`VolumeShape::total_voxels`, `checked_linear_index`, block validation), but not universally. Agents should maintain defensive arithmetic when computing byte offsets and buffer sizes.

## External References

- **MRC-2014 Spec**: `mrcfile-official.md` (local copy) or https://www.ccpem.ac.uk/mrc-format/mrc2014/
- **Python Reference**: `mrcfile/` directory (CCP-EM's `mrcfile` Python package)
