# Agent Guide for `mrc`

This file contains project-specific context for AI coding agents working on the `mrc` crate.

- **Repository**: https://github.com/elemeng/mrc
- **Crate**: https://crates.io/crates/mrc
- **Version**: 0.8.0 (check `Cargo.toml`)
- **Language**: Rust, Edition 2024, MSRV 1.85
- **Hard deps**: `thiserror` 2.x, `tracing` 0.1
- **Spec reference**: `mrcfile-official.md` (local copy)

### CLI crate

The [`mrc-cli`](https://crates.io/crates/mrc-cli) crate provides the `mrc-cli` binary (12 subcommands).
It depends on `mrc` (path) plus `clap` 4.5 and `image` 0.25.

```bash
# Build CLI
(cd cli && cargo build)

# Install CLI
cargo install --path cli
# or: cargo install mrc-cli
```

## Build and Test Commands

```bash
cargo build                    # default features (mmap, f16, simd, parallel, gzip)
cargo build --all-features     # recommended for development
cargo test --all-features      # unit + integration + doc-tests
cargo clippy --all-features
cargo fmt --check
cargo doc --all-features       # verify no broken intra-doc links
cargo bench --all-features     # run all benchmarks (requires [[bench]] harness=false in Cargo.toml)
```

Unit tests: inline `#[cfg(test)]` modules. Integration tests: `tests/integration.rs` (~23 roundtrip tests). Comprehensive API tests: `tests/api_comprehensive.rs` (~67 tests covering every public API). Regression tests: `tests/regressions.rs` (~25 tests for alignment, endianness, write validation, rounding, IMOD Mode 0, and finalize checks). Real-data tests: `tests/real_data_tests.rs` (~77 tests on 6 real MRC files from `real_data/`). Benchmarks: `benches/bench.rs` (criterion, ~49 benchmarks including 25 tomogram benchmarks).

CI additionally runs a feature matrix (`--no-default-features` plus each optional feature), `clippy --all-targets -D warnings`, rustdoc with `-D warnings`, and `cargo miri test --lib`.

All commands should be prefixed with `rtk` when available (e.g. `rtk cargo test`).

## Cargo.toml Notes

- `[[bench]] name = "bench" harness = false` is required for criterion benchmarks. Without this, `cargo bench` runs 0 benchmarks.

## Code Organization

```
src/
├── lib.rs                 # Public API re-exports and convenience functions (open, create)
├── error.rs               # Error and HeaderValidationError enums (thiserror)
├── mode.rs                # Mode enum, Voxel trait, complex types, Packed4Bit
├── header/
│   ├── mod.rs             # Header struct (1024-byte MRC-2014 header), HeaderBuilder
│   ├── fei.rs             # FEI1/FEI2 extended header parsers
│   ├── ccp4.rs            # CCP4 symmetry record parser
│   ├── mrco.rs            # MRCO legacy record parser
│   ├── seri.rs            # SerialEM record parser
│   └── agar.rs            # Agard record parser
├── validate.rs            # ValidationReport, validate_full(), validate_reader()
├── serde_byte_array.rs    # (private) serde helpers for byte arrays > 32
├── iter.rs                # Lazy iterators: RegionIter, SliceStepper, SlabStepper, TileStepper
├── engine/
│   ├── block.rs           # VolumeShape, VoxelBlock<T>
│   ├── codec.rs           # EndianCodec trait, decode_slice, encode_slice, encode_block_parallel
│   ├── convert.rs         # Type conversion utilities, convert_block, ConvertFrom trait
│   ├── endian.rs          # FileEndian enum, MachstInfo
│   ├── simd/              # AVX2/NEON SIMD kernels (x86.rs, aarch64.rs)
│   └── stats.rs           # Statistics computation and header stats validation
├── io/
│   ├── reader.rs          # Reader (auto-selects mmap/buffered)
│   ├── reader_common.rs   # Block validation, gather helpers, decompression, parse_header, ConvertReader
│   ├── writer.rs          # Writer, WriterBuilder (single Writer type for all backends)
│   ├── gzip.rs            # impl Reader { open_gzip* }
│   └── bzip2.rs           # impl Reader { open_bzip2* }
tests/
    └── integration.rs     # ~23 roundtrip tests
```

### Module Philosophy

- `engine/` — format-agnostic encoding/decoding primitives (no I/O knowledge)
- `io/` — user-facing I/O strategies: buffered, mmap, compressed
- `iter/` — lazy iterators parameterized by a Stepper strategy

### API Discipline

The main public entry point is `lib.rs`. Internal modules are `mod` (private) or `pub mod` with `#[doc(hidden)]`:

| Visibility | Items |
|------------|-------|
| **Public** | `open`, `create`, `Reader`, `ConvertReader`, `WriterBuilder`, `Writer`, `Header`, `HeaderBuilder`, `Mode`, `Voxel`, `VoxelBlock`, `VolumeShape`, `DataView`, `DataBlock`, `OwnedData`, `FileEndian`, `Error`, `HeaderValidationError`, `CompressionLevel`, validate types, FEI/CCP4/MRCO/SERI/AGAR/IMOD types, ExtHeaderType/Data, conversion utilities, `DEFAULT_MAX_DECOMPRESSED_BYTES`, `raw_bytes`, `read_block_bytes`, `read_block_bytes_cow` |
| **`#[doc(hidden)]`** | `EndianCodec`, `MachstInfo`, `CompressionType`, `detect_compression`, `EndianFallbackWarning`, `serde_byte_array` |
| **`pub(crate)` only** | `RegionIter`, `SliceStepper`, `SlabStepper`, `TileStepper`, `validate_block_bounds`, `gather_block_bytes`, `decode_block`, `decode_slice`, `encode_slice`, `convert_block`, `decode_block_to_any`, `parse_header`, `open_compressed`, `compute_stats`, `validate_header_stats`, SIMD wrapper functions, converter functions |

- `read_block_bytes_cow` is publicly exposed for advanced users needing zero-copy raw byte access (contiguous XY slabs return `Cow::Borrowed`). Its doc comment warns that bytes are raw on-disk bytes — no endian correction, no type conversion.
- `read_block_bytes` always copies — includes a performance warning in its doc comment.

## Real Data Files

6 real MRC files in `real_data/`:

| File | Mode | Dims | Size |
|------|------|------|------|
| `search.mrc` | Int16 | 1440×1022×14 | 39 MB |
| `Atlas.mrc` | Int16 | 1440×1022×47 | 133 MB |
| `preview.mrc` | Int16 | 1440×1022×77 | 216 MB |
| `box320_bin2_mask.mrc` | Float32 | 320×320×320 | 125 MB |
| `em-map.mrc` | Float32 | 320×320×320 | 125 MB |
| `tomogram.mrc` | Float32 | 1022×1440×750 | 4.2 GB |

## Benchmark Structure

`benches/bench.rs` has two criterion groups:
- `benches` — synthetic benchmarks + non-tomogram real data (sample_size=50)
- `tomo_benches` — all tomogram benchmarks (sample_size=10, measurement_time=30s)

Filter with: `cargo bench -- "tomo"` or `cargo bench -- "tomogram/validate"`.

Key enums (`Error`, `Mode`, `CompressionLevel`, `CompressionType`, `ComplexToRealStrategy`, `M0Interpretation`, `ExtHeaderType`, `ExtHeaderData`) are `#[non_exhaustive]`.

## Development Conventions

### Code Style

- `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::perf))]`
- `#![warn(missing_docs, clippy::cargo)]`
- No `.unwrap()` or `.expect()` in production code
- All public items must have doc comments
- Small accessors and hot-path conversions marked `#[inline]`
- Error paths and `bounds_err()` helpers marked `#[cold]`

### Error Handling

- All fallible functions return `Result<T, Error>`.
- `Error` is a central `thiserror` enum: `Io`, `InvalidHeader`, `UnsupportedMode`, `BoundsError`, `TypeMismatch`, `BlockShapeMismatch`, `ModeMismatch`, `InvalidHeaderDetailed`, `StatsMismatch`, `Mmap`, `FileSizeMismatch`, `NotAVolumeStack`, `ValueOutOfRange`.
- `HeaderValidationError` provides granular header diagnostics.
- `ModeMismatch`, `TypeMismatch`, `ValueOutOfRange` preferred over silent data corruption.

### Type Safety

- `Voxel` trait connects Rust types to MRC modes at compile time for the typed `ConvertReader` and writer APIs.
- Default reader methods (`slices`, `slabs`, `tiles`, `subregion`, `read_volume`, `volumes`) are **non-generic** — they return `DataBlock` whose `DataView` variant is determined at runtime by the file's mode. This avoids mode-mismatch errors at the cost of a runtime match.
- `Packed4Bit` (Mode 101) has no `Voxel` impl — use `slices_u8`/`read_volume_u8`/`write_u4_block`.
- No `unsafe` in the public API — all `unsafe` is internal.

## Safety and Unsafe Code

Unsafe locations and their justifications:

1. **`engine/simd/x86.rs` + `aarch64.rs`** — AVX2/NEON intrinsics. Runtime feature detection via `is_x86_feature_detected!("avx2")` / `is_aarch64_feature_detected!("neon")`. All `unsafe fn` bodies require explicit `unsafe { }` blocks (Rust 2024 `unsafe_op_in_unsafe_fn` lint).
2. **`io/reader.rs`** — `memmap2::Mmap` / `MmapMut` construction and `DataBlock::Borrowed` zero-copy view (mmap and buffered). Alignment, mode, and endianness checked before pointer dereference.
3. **`engine/codec.rs`** — `core::ptr::copy_nonoverlapping` for native-endian memcpy; `Vec::set_len` after capacity-guaranteed initialization.
4. **`engine/convert.rs`** — `reinterpret_vec` and `Vec::from_raw_parts` for type-erased Vec reuse. Type identity verified via `TypeId` before transmute.

All `unsafe` blocks must have a `// SAFETY:` comment documenting the invariant.

## Known Issues and Technical Debt

1. **`gather_block_bytes` fast-path assumes origin-aligned XY slabs**: full-row slabs (`ox == 0 && sx == nx && oy == 0 && sy == ny`) use a contiguous copy. Sub-XY blocks correctly scatter/gather row-by-row.
2. **`Reader::raw_bytes()` silently truncates on undersized files in permissive mode**: returns available bytes. Use `is_truncated()` to detect. Strict mode validates on open.
3. **`Packed4Bit` sub-block reads require even X-offset**: `validate_block_bounds` rejects odd `ox` for Mode 101. Full-frame and byte-aligned sub-blocks work.
4. **`write_block_as_body` f32 clone eliminated** in v0.5.0 — existing code paths are clean.
5. **`DataBlock::data()` borrows from reader or `OwnedData`**: the returned `DataView` has the block's lifetime, so chaining `reader.read_volume()?.data()` produces a temporary — bind the block first. This is a deliberate lifetime choice that prevents dangling references.

## Roadmap

- **v0.3.x** ✅ — Complete MRC-2014 format support, iterator API, SIMD, mmap, all modes
- **v0.4.x** ✅ — Serde, tracing, dispatch enums, IMOD expansion, Miri CI, clippy, error context
- **v0.5.x** ✅ — API completeness (`header_mut`, builder setters), `is_truncated` fix, perf cleanup, doc overhaul
- **v0.6.x** — Robust real-world testing across all public APIs with real MRC files
- **v0.8.x** — Hardening: alignment-safe typed conversion, writer mode/length validation, per-component complex byte swap, bounded/streaming decompression, Miri + feature-matrix CI

## When Modifying This File

If you modify any files, styles, structures, configurations, workflows, or other conventions mentioned in this guide, update the corresponding sections of this file to keep it current.
