# Agent Guide for `mrc`

This file contains project-specific context for AI coding agents working on the `mrc` crate. The reader is assumed to know nothing about the project.

## Project Overview

`mrc` is a Rust library crate that reads and writes MRC-2014 files, a binary format used in cryo-electron microscopy (cryo-EM) and structural biology. The crate prioritizes zero-copy access, type-safe I/O, and SIMD acceleration for common data conversion paths.

- **Repository**: https://github.com/elemeng/mrc
- **Crate**: https://crates.io/crates/mrc
- **Docs**: https://docs.rs/mrc
- **License**: MIT
- **Version**: 0.3.0 (check `Cargo.toml` for latest)

A reference Python implementation (`mrcfile`) is available on PyPI for specification comparison, but this crate is a standalone Rust implementation. The MRC-2014 specification is available locally as `mrcfile-official.md`.

## Technology Stack

- **Language**: Rust, Edition 2024, MSRV 1.85
- **Build Tool**: Cargo (no `rust-toolchain.toml` — uses system Rust)
- **CI**: GitHub Actions (`.github/workflows/rust.yml`) — builds and tests on `ubuntu-latest` for pushes/PRs to `main`
- **Error Handling**: `thiserror` 2.x (no-std compatible)
- **No `unsafe` in public API**: All `unsafe` is internal; the public API is 100% safe Rust.
- **Optional Dependencies** (as declared in `Cargo.toml`):
  - `memmap2` 0.9 — memory-mapped I/O (`mmap` feature)
  - `rayon` 1.12 — parallel encoding (`parallel` feature)
  - `half` 2.7 — half-precision f16 (`f16` feature)
  - `flate2` 1.1 — gzip compression (`gzip` feature)
  - `bzip2` 0.5 — bzip2 compression (`bzip2` feature)

## Build and Test Commands

```bash
# Build with default features (mmap, f16, simd, parallel, gzip)
cargo build

# Build with all features (recommended for development)
cargo build --all-features

# Run all tests (unit + doc tests)
cargo test --all-features

# Run tests with only default features
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features

# Build release binaries
cargo build --release --bin mrc-validate
cargo build --release --bin mrc-header
cargo build --release --bin mrc-invert
```

Unit tests are inline `#[cfg(test)]` modules inside source files; integration tests live in `tests/integration.rs`; and benchmarks are in `benches/bench.rs` (criterion).

## Feature Flags

| Feature | Dependencies | Description | Default |
|---------|-------------|-------------|---------|
| `mmap` | `memmap2` | Memory-mapped readers/writers | ✅ |
| `f16` | `half` | Half-precision `f16` support | ✅ |
| `simd` | (none; uses `core::arch`) | AVX2/NEON accelerated conversions (`i16→f32`, etc.) | ✅ |
| `parallel` | `rayon` | Parallel encoding for `write_block_parallel` | ✅ |
| `gzip` | `flate2` | Gzip-compressed MRC I/O | ✅ |
| `bzip2` | `bzip2` | Bzip2-compressed MRC I/O | ❌ |
| `ndarray` | `ndarray` | Return volumes as `ndarray::Array3` via `to_ndarray()` | ❌ |

The `simd` feature uses **runtime feature detection** (`is_x86_feature_detected!("avx2")` / `is_aarch64_feature_detected!("neon")`) — it never assumes the ISA is available at compile time. Scalar fallbacks are always present.

## Code Organization

```
src/
├── lib.rs                 # Public API re-exports and convenience functions (open, create)
├── error.rs               # Top-level `Error` and `HeaderValidationError` enums (thiserror)
├── mode.rs                # `Mode` enum, `Voxel` trait, complex types (Int16Complex, Float32Complex), Packed4Bit mode handling
├── header/
│   ├── mod.rs             # `Header` struct (1024-byte MRC-2014 header), `HeaderBuilder`
│   ├── fei.rs             # FEI1/FEI2 extended header parsers
│   ├── ccp4.rs            # CCP4 symmetry record parser
│   ├── mrco.rs            # MRCO legacy record parser
│   ├── seri.rs            # SerialEM record parser (with alpha_tilt)
│   └── agar.rs            # Agard record parser
├── validate.rs            # `ValidationReport`, `validate_full()`, `validate_reader()`
├── iter.rs                # Lazy iterators: `RegionIter<T, R, S>`, `SliceStepper`, `SlabStepper`, `TileStepper`
├── engine/
│   ├── mod.rs
│   ├── block.rs           # `VolumeShape`, `VoxelBlock<T>`
│   ├── codec.rs           # `EndianCodec` trait, `decode_slice`, `encode_slice`, parallel `encode_block_parallel`
│   ├── convert.rs         # Type conversion utilities (i16→f32, u16→f32, i8→f32, u8↔u16, Mode 0 reinterpretation, 4-bit unpacking)
│   ├── endian.rs          # `FileEndian` enum, `MachstInfo` metadata
│   ├── simd.rs            # AVX2/NEON SIMD kernels (unsafe) — i8→f32, i16→f32, u16→f32
│   └── stats.rs           # Statistics computation (dmin, dmax, dmean, rms), header statistics validation
├── io/
│   ├── mod.rs
│   ├── reader.rs          # `CompressionType` and `detect_compression` helpers
│   ├── reader_common.rs   # Shared `VoxelSource` trait, `ReaderCore` trait, block validation, gather/encode helpers, `parse_header`, `open_compressed`
│   ├── buffered.rs        # In-memory `Reader` (loads entire file into Vec<u8>)
│   ├── mmap_reader.rs     # `MmapReader` (zero-copy, requires `mmap` feature)
│   ├── writer.rs          # `Writer`, `WriterBuilder`, `MmapWriter`, `CompressedWriter<C: Compressor>`, `Compressor` trait
│   ├── gzip.rs            # `GzipCompressor`, `GzipWriter` type alias, gzip reader methods on Reader
│   └── bzip2.rs           # `Bzip2Compressor`, `Bzip2Writer` type alias, bzip2 reader methods on Reader
└── bin/
    ├── mrc-validate.rs    # CLI validation tool — comprehensive file validation with field filtering (`--field`)
    ├── mrc-header.rs      # CLI header inspector — key:value output with inline validation (`--force` to skip)
    └── mrc-invert.rs      # CLI contrast inverter — negates all voxel values, writes Float32 output
```
tests/
    └── integration.rs    # ~21 integration tests (roundtrip, compression, edge cases)

### Module Philosophy

- `engine/` contains low-level, format-agnostic encoding/decoding primitives.
- `io/` contains user-facing I/O strategies (buffered, mmap, compressed).
- `iter/` (a single `iter.rs` file, not a directory) provides lazy iterators that work over any `VoxelSource` implementor.
- The crate uses **sealed traits** (`VoxelSource`) to keep internal abstractions internal.
- `Packed4Bit` (Mode 101) has no `Voxel` impl. Read via `read_volume_u8()`/`slices_u8()` which unpack nibbles to `u8`; write via `write_u4_block()` which packs `u8` values (0–15) two-per-byte.

### I/O Strategies

| Reader | Description | Best for |
|--------|-------------|---------|
| `Reader` (buffered) | Loads entire file into `Vec<u8>` on open | Smaller files, random access |
| `MmapReader` | Memory-maps the file, OS-managed paging | Large files, partial reads, zero-copy `slab_as` |

| Writer | Description | Best for |
|--------|-------------|---------|
| `Writer` | Standard file I/O, writes blocks directly to disk | General use |
| `MmapWriter` | Memory-mapped write via `memmap2::MmapMut` | Very large files (`mmap` feature) |
| `GzipWriter` | Buffers in RAM, compresses on `finalize` | Compressed output (`gzip` feature) |
| `Bzip2Writer` | Buffers in RAM, compresses on `finalize` | Compressed output (`bzip2` feature) |

File open auto-detects gzip/bzip2 from magic bytes: `\x1f\x8b` → gzip, `BZ` → bzip2, anything else → plain.

### API Surface Discipline

The top-level `lib.rs` is the *only* public entry point. Internal modules (`engine/`, `io/`, `iter/`, `fei/`) are marked `mod` (private) or, when their items must be re-exported, are `pub mod` but with `#[doc(hidden)]` on internal plumbing:

| Visibility | Items |
|------------|-------|
| **Public (in lib.rs)** | `open`, `create`, `Reader`, `WriterBuilder`, `Writer`, `Header`, `HeaderBuilder`, `Mode`, `Voxel`, `VoxelBlock`, `VolumeShape`, `RegionIter`, `SliceStepper`, `SlabStepper`, `TileStepper`, `FileEndian`, `Error`, `HeaderValidationError`, `MmapReader`, `MmapWriter`, `GzipWriter`, `Bzip2Writer`, `validate_full`, `validate_reader`, `ValidationReport`, `ValidationIssue`, `Severity`, FEI types, CCP4/MRCO/SERI/AGAR types, `ComplexToRealStrategy`, `M0Interpretation`, `Int16Complex`, `Float32Complex`, `convert_u8_slice_to_u16`, `convert_u16_slice_to_u8`, `reinterpret_m0`, `DEFAULT_MAX_DECOMPRESSED_BYTES`, `ReaderMethods`, `ConvertMethods` |
| **`#[doc(hidden)]`** | `VoxelSource`, `ReaderCore`, `EndianCodec`, `Compressor`, `MachstInfo`, `CompressionType`, `detect_compression`, `GzipCompressor`, `Bzip2Compressor`, `EndianFallbackWarning` |
| **`pub(crate)` only** | `validate_block_bounds`, `gather_block_bytes`, `encode_block_to_buf`, `decode_block`, `decode_native_endian`, `decode_slice`, `encode_slice`, `encode_block_parallel`, `parse_header`, `DecompressedMrc`, `open_compressed`, `compute_stats`, `validate_header_stats`, `unpack_u4_bytes_to_u8`, `convert_i8_slice_to_f32`, `convert_i16_slice_to_f32`, `convert_u16_slice_to_f32`, `convert_f32_slice_to_i16`, `convert_f32_slice_to_u16`, `convert_f32_slice_to_i8` |

## Usage Note — Trait Imports (Optional)

Iterator and conversion methods (`slices`, `slabs`, `subregion`, `read_volume`,
`convert`, etc.) are defined on the [`ReaderMethods`] and [`ConvertMethods`]
traits respectively, with **inherent forwarding methods** on `Reader` and
`MmapReader`.  For normal use **no trait import is needed**:

```rust
use mrc::Reader;

let reader = Reader::open("file.mrc")?;
for slice in reader.slices::<f32>() { ... }
for slice in reader.convert::<f32>().slices() { ... }
```

Import the traits explicitly only when writing generic code over reader types:

```rust
use mrc::{Reader, ReaderMethods, ConvertMethods};

fn process<T, R>(reader: &R)
where R: ReaderMethods + ConvertMethods + ... { ... }
```

## Development Conventions

### Code Style

- **Language**: All comments, docs, and identifiers are in English.
- **Formatting**: Standard `rustfmt`. No custom `rustfmt.toml`.
- **Clippy**: The crate enforces `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::perf))]` in `lib.rs`. Production code must not use `.unwrap()` or `.expect()`, and performance lints are errors.
- **Inlining**: Small accessor methods and hot-path conversion functions are marked `#[inline]`.
- **Documentation**: Heavy use of `//!` module docs and `///` item docs. The crate-level doc (`lib.rs`) includes real-world workflow examples (tilt series, FEI metadata, volume stacks) and a troubleshooting table. Doc-tests are present and run with `cargo test` — ~33 doc-tests across `lib.rs`, `header.rs`, `validate.rs`, `error.rs`, `io/buffered.rs`, `io/writer.rs`, `io/mmap_reader.rs`, and `engine/codec.rs`.

### Error Handling

- Fallible functions return `Result<T, Error>`.
- `Error` is a central enum using `thiserror` for `#[from]` conversions.
- Specific error variants: `Io`, `InvalidHeader`, `UnsupportedMode`, `BoundsError`, `TypeMismatch`, `BlockShapeMismatch`, `ModeMismatch`, `InvalidHeaderDetailed`, `StatsMismatch`, `Mmap`, `FileSizeMismatch`, `NotAVolumeStack`.
- `HeaderValidationError` provides granular diagnostics for header validation.
- `ModeMismatch` and `TypeMismatch` errors are preferred over silent data corruption.
- Bounds checking on `VoxelBlock` shapes is mandatory.

### Type Safety

- The `Voxel` trait connects Rust types to MRC modes at compile time.
- Generic read/write APIs require `T: Voxel`, preventing runtime mode mismatches.
- Built-in conversion conveniences: `reader.convert::<T>()` adapter for auto-conversion, `slices_u8()`, `slabs_u8()`, `slices_mode0()`, `slabs_mode0()`, `read_volume::<T>()`, `read_volume_u8()`, `write_block_as()`, `write_u8_block()`, and `write_u4_block()`.

### MRC Mode Mapping

| Mode constant | Rust type | `Voxel` impl | Typical use |
|---------------|-----------|--------------|-------------|
| 0 (Int8) | `i8` | ✅ | Binary masks |
| 1 (Int16) | `i16` | ✅ | Raw cryo-EM density |
| 2 (Float32) | `f32` | ✅ | Processed/reconstructed density |
| 3 (Int16Complex) | `Int16Complex` | ✅ | Complex data (obsolete for writing) |
| 4 (Float32Complex) | `Float32Complex` | ✅ | Complex data |
| 6 (Uint16) | `u16` | ✅ | Segmentation labels |
| 12 (Float16) | `f16` (via `half` crate, feature `f16`) | ✅ | Half-precision storage |
| 101 (Packed4Bit) | `u8` (via `slices_u8`/`read_volume_u8`) | ❌ (no Voxel impl) | 4-bit packed data; use `convert::<f32>().slices()` for f32 conversion |

### File Endianness

- MRC files encode byte order via the 4-byte MACHST machine stamp at offset 212.
- Standard stamps: `0x44 0x44 0x00 0x00` = little-endian; `0x11 0x11 0x00 0x00` = big-endian.
- CCP4 variant: `0x44 0x41` = little-endian.
- New files are always little-endian per crate policy (matching Python `mrcfile`).
- Header decode has an endianness fallback: if MODE is invalid under the detected endianness, the opposite is tried. This handles files with a wrong MACHST.

### Header Validation

- `Header::validate_detailed()` enforces strict MRC-2014 compliance. Since v0.2.5 it also accepts NVERSION=0 (uninitialized, common in EPU microscope files) alongside 20140/20141, so `open()` works on EPU data without special flags.
- `Header::validate_permissive()` turns most non-fatal issues into warnings.
- `validate_map()` accepts `"MAP "`, `"MAP\0"`, `"MAPI"`, and all-zero MAP fields — covering EPU, IMOD, and uninitialized headers.

## Testing Strategy

- **Unit Tests**: ~91 tests in inline `mod tests` blocks inside source files (`header.rs`, `engine/simd.rs`, `engine/convert.rs`, `engine/endian.rs`, `engine/stats.rs`, `io/reader.rs`, `lib.rs`, `mode.rs`).
- **Doc Tests**: ~39 doc-tests (33 run, 6 ignored — for internal-only API patterns) in `lib.rs`, `header.rs`, `validate.rs`, `error.rs`, `io/buffered.rs`, `io/writer.rs`, `io/mmap_reader.rs`, and `engine/codec.rs`.
- **Integration Tests**: ~21 integration tests in `tests/integration.rs` covering write-then-read roundtrips for Float32, Int16, Uint16, subregion reads, gzip/bzip2 compression, header statistics, MmapReader, Packed4Bit (Mode 101), complex modes, volume stacks, and permissive-mode edge cases.
- **Benchmarks**: Criterion benchmarks live in `benches/bench.rs` (requires `--all-features` for mmap benchmarks).
- **No External Fixtures**: Tests generate temporary MRC files programmatically (using `tempfile` in dev-dependencies) rather than checking large binary files into git.

## Safety and Unsafe Code

The crate contains a small amount of `unsafe` Rust, all justified by performance:

1. **SIMD Kernels** (`engine/simd.rs`): AVX2 and NEON intrinsics for `i8→f32`, `i16→f32`, `u16→f32`. Runtime feature detection gates these.
2. **Memory Mapping** (`io/mmap_reader.rs`, `io/writer.rs`): `memmap2::MmapOptions::new().map()` and `.map_mut()` require `unsafe`.
3. **Fast-path memcpy** (`engine/codec.rs`): `core::ptr::copy_nonoverlapping` is used for native-endian decode/encode to avoid per-element branching.
4. **`Vec::set_len`** (`engine/codec.rs`): Used after `Vec::with_capacity` when all elements are guaranteed to be overwritten immediately.
5. **Zero-copy `slab_as`** (`io/mmap_reader.rs`): `core::slice::from_raw_parts` returns a `&[T]` into the memory map. Alignment, mode, and endianness are checked beforehand.

**Agent Guidance**: When modifying unsafe code, ensure:
- Runtime feature detection for SIMD (do not assume AVX2/NEON is available).
- Alignment and size invariants are documented with `// SAFETY:` comments.
- No undefined behavior is introduced through out-of-bounds raw pointer access.
- `Vec::set_len` is only called after all elements in the allocated capacity are initialized.

## Known Issues and Technical Debt

1. **`gather_block_bytes` fast-path assumes origin-aligned XY slabs**: For full-row slabs (`ox == 0 && sx == nx && oy == 0 && sy == ny`) a contiguous copy is used. Sub-XY blocks correctly use row-by-row scatter/gather. The fast-path comment was clarified in v0.2.7.
2. **`MmapReader::data_bytes()` silently truncates on undersized files in permissive mode**: When the file is smaller than the header claims, the method returns whatever bytes are available. Use [`is_truncated()`](crate::MmapReader::is_truncated) to detect this. In strict mode the file size is validated on open.
3. **`Packed4Bit` sub-block reads require even X-offset**: `validate_block_bounds` rejects odd `ox` for Mode 101 to avoid nibble-level read-modify-write in `gather_block_bytes`. Full-frame and byte-aligned sub-block reads work correctly.

## CLI Tools

Three binary targets are available (`src/bin/`):

- **`mrc-validate`**: Comprehensive file validation. Supports `--permissive` (warnings instead of hard errors), `--field <name>` (filter to specific checks), and `--list-fields`. Exit code 0 = valid, 1 = validation failed, 2 = usage error.
- **`mrc-header`**: Header inspector with key:value output. Uses `--permissive` for lenient opening, `--force` to skip validation and show raw values only.
- **`mrc-invert`**: Contrast inverter. Reads any mode via `convert::<f32>().slices()`, negates every voxel, writes Float32 output. Shows progress every 100 slices.

## Deployment and Release

- The crate is published to **crates.io**.
- `cargo build --release` produces optimized artifacts.
- CI only runs on Ubuntu; there is no cross-platform or Windows/macOS testing in CI.
- The CI workflow builds with `--release` and runs tests (without `--all-features`).

## Security Considerations

- **File Size Validation**: Readers validate that file size matches header-declared data size (with a `FileSizeMismatch` error) unless opened in permissive mode.
- **Memory Mapping**: `MmapReader` maps files read-only. `MmapWriter` maps read-write and can mutate files in place.
- **Compression**: Gzip/Bzip2 readers decompress the entire file into memory on open (they do not stream). A hard cap of [`DEFAULT_MAX_DECOMPRESSED_BYTES`](crate::DEFAULT_MAX_DECOMPRESSED_BYTES) (256 GiB) is enforced before the header is parsed, preventing decompression bombs. Use [`open_gzip_with_limit`](crate::Reader::open_gzip_with_limit) / [`open_bzip2_with_limit`](crate::Reader::open_bzip2_with_limit) for a custom limit, or set to `u64::MAX` to disable the cap.
- **No `unsafe` in public API**: All `unsafe` is internal; the public API is 100% safe Rust.
- **Integer Overflow**: The codebase uses `checked_mul` and `checked_add` for size calculations in several places (`VolumeShape::total_voxels`, `checked_linear_index`, block validation), but not universally. Agents should maintain defensive arithmetic when computing byte offsets and buffer sizes.

## External References

- **MRC-2014 Spec**: `mrcfile-official.md` (local copy) or https://www.ccpem.ac.uk/mrc-format/mrc2014/
- **Python Reference**: CCP-EM's `mrcfile` Python package

## When Modifying This File

If you modify any files, styles, structures, configurations, workflows, or other conventions mentioned in this guide, update the corresponding sections of this file to keep it current.
