# Update Log

## 2026-07-16 — v0.6.0 → v0.7.0: Runtime DataView dispatch, removed generic Reader API

~12 files changed, +832/−706. This release replaces the generic reader methods with
runtime dispatch via `DataView`/`DataBlock`, eliminating ModeMismatch errors on the
read path.

### Breaking Changes

| Old (v0.6.x) | New (v0.7.0) |
|-----|-----|
| `reader.slices::<f32>()` | `reader.slices()` → match on `DataView::Float32` |
| `reader.read_volume::<T>()` | `reader.read_volume()` → match on `DataView` |
| `reader.subregion::<T>(...)` | `reader.subregion(...)` → match on `DataView` |
| `reader.volumes::<T>()` | `reader.volumes()` → match on `DataView` |
| `reader.slabs::<T>(k)` | `reader.slabs(k)` → match on `DataView` |
| `reader.tiles::<T>(shape)` | `reader.tiles(shape)` → match on `DataView` |
| `reader.slab_as::<T>(z, k)` | Removed. Use `reader.subregion([0,0,z], [nx,ny,k])` instead |

**What changed:** The default reader methods are no longer generic over `T:
Voxel`. Instead they return `DataBlock<'_>` whose `DataView` variant is
determined at runtime by the file's mode. Users match on the enum variant
(`DataView::Float32(data)` / `DataView::Int16(data)` / etc.) instead of
specifying a compile-time type.

This means:
- No more `ModeMismatch` errors when reading — the file's mode always
  determines the returned `DataView` variant.
- No need to know the file's mode ahead of time — just call `.slices()`
  and match.
- The writer and `ConvertReader` APIs remain generic (`T: Voxel`) and
  unchanged.

### New Types

| Type | Description |
|------|-------------|
| `DataView<'a>` | Typed reference slice (Int8, Int16, Float32, Int16Complex, Float32Complex, Uint16, Float16, Packed4Bit) |
| `DataBlock<'a>` | Block with `offset()`, `shape()`, `data()` methods, either Borrowed (zero-copy) or Owned |
| `OwnedData` | Owned variant of DataView |

### Additions

- **`decode_block_to_any()`** — decodes `&[u8]` to `OwnedData` by matching on
  `Mode` at runtime, used by `RegionIter` and `subregion`
- **`WriterBuilder::finish_buffer()`** — builds an in-memory writer backed by
  `Cursor<Vec<u8>>`, consistent with `finish_mmap`/`finish_gzip`/`finish_bzip2`
- **`Header` convenience table** — full list of 32+ header methods in
  crate-level documentation
- **`NotAVolumeStack`, `BlockShapeMismatch`** — added to error troubleshooting tables

### Removals

- `Reader::slab_as()` — zero-copy typed access is now implicit in `DataBlock::Borrowed`

### Documentation

- All doc examples updated to use non-generic API and `DataView` pattern matching
- `APIs.md` — corrected Reader method signatures, added `DataBlock`/`DataView` types
- `AGENTS.md` — `RegionIter`/steppers moved to `pub(crate)`, added `DataView`/`DataBlock`/`OwnedData`
- Writer backend table simplified to show only builder methods

### Testing

- 402 tests (98 unit + 67 api_comprehensive + 23 integration + 214 doc-tests)
- All pass cleanly across --all-features builds, 0 clippy warnings

---

## 2026-07-09 — API naming cleanup, ergonomic improvements, semantic fixes

~11 files changed, +628/−109. This batch focuses on naming consistency, one-shot ergonomics,
and corner-case correctness.

### Naming (backward-incompatible for 0.x)

| Old | New | Reason |
|-----|-----|--------|
| `WriterBuilder::set_volume_stack(mz)` | `.volume_stack(mz)` | Builder bare-noun convention (all other builder methods are bare nouns) |
| `WriterBuilder::set_image_stack()` | `.image_stack()` | Same |
| `WriterBuilder::set_volume()` | `.volume()` | Same |
| `Compression` (enum) | `CompressionLevel` | Distinguish from `CompressionType` (auto-detection) |
| `WriterBuilder::ext_header_bytes()` | `.extended_header()` | Setter name reads as setter, not getter |
| `Reader::data_bytes()` | `raw_bytes()` | Clarifies "raw file bytes, not decoded typed view" |

### Ergonomic Additions

- **`Writer::set_data(&[T])`** — write full volume + compute stats in one call.
  Shape taken from pre-configured `Writer`; uses `checked_mul` to guard against
  overflow (matching `VolumeShape::total_voxels`).
- **`read_as::<T>(path)`** — one-shot: `open` + `read_volume`, returns `(Header, Vec<T>)`.
- **`write_as(path, data, shape)`** — one-shot: `create` + `set_data` + `finalize`.
- **`Reader::is_single_image()` / `is_image_stack()` / `is_volume()` / `is_volume_stack()`** —
  delegation to `Header`, avoiding `reader.header().is_volume_stack()`.
- **`Reader::logical_shape()`** — 4D shape `[nvolumes, mz, ny, nx]`.
- **`ConvertReader::volumes()`** — iterate sub-volumes with auto-conversion; returns
  `Err(NotAVolumeStack)` on non-stack files, matching `Reader::volumes()`.

### Semantic Fixes

- **`Writer::finalize()` — `finalized` flag set after I/O, not before.** Previously
  `self.finalized = true` was written before any I/O. If disk-write failed, the Drop
  guard would silently suppress the warning. Now `finalized = true` is set only after
  all I/O succeeds (line 1447: `if result.is_ok() { self.finalized = true; }`).
- **`Writer::set_data()` — `checked_mul` for volume size.** `nx * ny * nz` could
  overflow on 32-bit. Now uses `checked_mul` chain like `VolumeShape::total_voxels`.
- **`Header::logical_shape()` — handle degenerate volume stack (mz=0).** Previously
  fell through to the `else` branch and returned `[1, nz, ny, nx]`, making a volume
  stack with mz=0 look like a single volume. Now returns `[0, 0, ny, nx]`, consistent
  with `Reader::volumes()` returning `Err(NotAVolumeStack)`.
- **`// SAFETY:` comments added** to both `slab_as` unsafe blocks (mmap + buffered),
  per AGENTS.md convention.
- **`write_block_as` doc** — first line now reads "Note: input data must be `f32`"
  to clarify the `VoxelBlock<f32>` signature constraint.

### Write‑side ergonomics

- **`Writer` Drop guard** — emits `tracing::warn!` (zero I/O) when dropped without
  calling `finalize()`. No data-loss risk; purely advisory.

### Testing

- 416 tests, clippy clean, doc clean.

### API Exposure

- **`ConvertReader`** — re-exported from crate root (was `pub` in a private module,
  invisible to docs). All 8 public methods now documented with doc comments.
- **`Fei1Metadata` / `Fei2Metadata`** — all 61 public fields now have doc comments
  (removed `#[allow(missing_docs)]`).

### Stale References Fixed

- **CLI binary name** — all docs (`src/lib.rs`, `README.md`, `AGENTS.md`) refer to
  the binary as `mrc-cli` rather than `mrc`, matching `Cargo.toml`.
- **`APIs.md`** — ConvertReader methods table expanded with `with_complex_strategy`,
  `with_m0_interpretation`, `to_ndarray` rows.
- **`AGENTS.md`** — `ConvertReader` added to Public API Surface table.

### Verification

- 0 doc warnings, 396 tests pass.

---

## 2026-07-08 — v0.4.1 → v0.5.0

~30 files changed. This release focuses on API completeness, documentation accuracy, and
code quality — closing gaps between the public API surface and what users actually need.

### API Additions

- **`Writer::header_mut()`** — mutable header access for mid-write modifications
  (previously only `&Header` was exposed)
- **Missing `WriterBuilder` setters** — `cell_angles()`, `nstart()`, `sampling()`,
  `axis_mapping()`, `add_label()` added to `builder_setters!()` macro, matching
  `HeaderBuilder`
- **`HeaderBuilder::mode_raw()`** — set raw mode constant for types without a `Voxel`
  impl (e.g. Packed4Bit)
- **`#[must_use]` on `WriterBuilder::ext_header_bytes`** — all builder methods now
  consistently annotated

### Bug Fixes

- **`is_truncated()` now works for buffered readers** — the `Buffered` variant was a
  tuple with no `truncated` flag; `is_truncated()` always returned `false` for
  permissive-mode buffered reads. Changed to struct variant with `truncated: bool`.
- **`write_u4_block` wrong error variant** — values > 15 returned `BoundsError`
  (geometry error) instead of the correct `ValueOutOfRange { value, max: 15 }`
- **`_read_from_buf` ext_header truncation** — extended header bytes were silently
  dropped when the input buffer was shorter than declared `ext_size`; now emits a
  warning in permissive mode
- **Broken intra-doc link** — ``[`Voxel`]`` in `HeaderBuilder::mode_raw` resolved
  via ``[`crate::Voxel`]``

### Performance

- **Eliminated f32 clone in `write_block_as_body`** — the Float32 pass-through arm
  cloned the entire `Vec<f32>` just to construct a temporary `VoxelBlock`. Extracted
  `write_block_data()` from `write_block()`, allowing direct slice-to-buffer encoding.
- **`write_u8_block` skips temporary `VoxelBlock`** — widened data written directly
  via `write_block_data::<u16>()` instead of building a `VoxelBlock<u16>` wrapper.

### Documentation Overhaul

- **`src/lib.rs`** — Restructured from 11 to 14 top-level sections, removing the
  "Advanced topics" grab-bag. Added richer examples: iteration patterns (`slices`,
  `slabs`, `tiles`), `write_block_as()` auto-conversion, `write_u8_block`/`write_u4_block`
  convenience methods, special-mode reads (`slices_u8`, `slices_mode0`),
  `FileEndian::from_machst()` detection, `validate_full()` report inspection, volume
  stack header configuration, error match example, `is_truncated()` detection.
  47 doc-tests (up from 41).
- **`README.md`** — Quick Start updated with `update_header_stats()` in the write
  example. Roadmap filled for v0.5.x. Stale badge refs cleaned.
- **`APIs.md`** — Fixed `RegionIter` lifetime, "backing by" typo, added `mode_raw` to
  `HeaderBuilder`, corrected feature flag descriptions.
- **`AGENTS.md`** — Updated simd path to `simd/` directory, removed stale file refs
  (`buffered.rs`, `mmap_reader.rs`, `VoxelSource`, `ReaderCore`), fixed `RegionIter`
  type params, corrected test counts throughout, removed non-existent type aliases
  (`MmapWriter`, `GzipWriter`, `Bzip2Writer`).

### Testing

- 98 unit tests, 23 integration tests, 47 doc-tests — all pass across debug, release,
  and --all-features builds. 0 clippy warnings, 0 doc warnings.

---

## 2026-07-07 — v0.3.x → v0.4.1

~3,200 lines changed across ~40 files. This release focuses on API quality, header ergonomics,
and safety hardening — with the first `serde` support, richer error diagnostics, and systematic
cleanup of technical debt.

### New Features

- **Optional `serde` support** — `Header`, `Mode`, `VolumeShape`, `ValidationReport` and all
  public types derive `Serialize`/`Deserialize` when the `serde` feature is enabled
- **Auto-dispatch extended header parsing** — `reader.parse_extended_header()` checks the
  4-byte `exttyp` field and routes to the correct parser (FEI1, FEI2, CCP4, MRCO, SERI, AGAR)
  automatically. Returns `ExtHeaderData` enum for generic code over all format types
- **Reader convenience methods** — one-call access to every extended header parser:
  `reader.fei1_metadata()`, `reader.ccp4_records()`, `reader.seri_records()`, etc.
- **`ExtHeaderType` enum** — identifies the extended header format without parsing data,
  enabling format-based dispatch in user code
- **Richer `Header` convenience API** — `header.cell_volume()`, `header.sampling()`,
  `header.density_stats()`, `header.label_at(i)`, `header.is_standard_map()`,
  `header.is_image_stack()`, `header.is_single_image()`, `header.nversion()`
- **Expanded IMOD metadata** — `ImodMetadata` now includes `image_type` (enum:
  Mono/Tilt/Tilts/Lina/Lins), `tilt_increment`, plus IMOD-origin detection in permissive mode
- **`ValueOutOfRange` error variant** — dedicated error for value-overflow conditions
  (replaces the previous misuse of `TypeMismatch` for u16→u8 narrowing)
- **MSRV CI job** — GitHub Actions validates builds against Rust 1.85 (declared MSRV)

### Breaking Changes

- **Removed deprecated `read_block()`** — both `Reader::read_block()` and
  `MmapReader::read_block()`, deprecated since v0.2.4, have been removed.
  Use `subregion()` instead (identical behaviour, available on all reader types).

### Safety & Correctness

- **Miri CI** — new GitHub Actions job runs `cargo miri test` on all unsafe code paths
  (SIMD kernels, mmap ops, `Vec::set_len`, pointer reinterpretation)
- **`#[must_use]` audit** — all builder methods and key accessors are annotated,
  preventing silent drops of un-finished writers
- **Richer error context** — `BoundsError` now carries optional `offset`, `shape`, and
  `volume` fields; `ModeMismatch` carries optional `offset` — significantly better
  diagnostics during debugging
- **`tracing::warn!` replaces `eprintln!`** — library diagnostics go through the
  `tracing` facade; users control output via their subscriber setup, not stderr
- **`ValueOutOfRange` added** — prevents overloading `TypeMismatch` for value-overflow
  errors; now used by `convert_u16_slice_to_u8()`

### Quality & Cleanup

- **Dead code removed** — `RunningStats` (entire struct + impl) moved under `#[cfg(test)]`;
  it was unused in production since v0.3's stats rewrite
- **Redundant imports purged** — `use std::vec::Vec` removed from 7 files (Vec is in the
  prelude since Rust 1.0)
- **`ok_or` → `ok_or_else`** — eager `bounds_err()` calls replaced with lazy evaluation
  in checked-arithmetic hot paths
- **Unreadable literals fixed** — digit separators added to floating-point constants
  throughout (`1.118034` → `1.118_034`, `300000.0` → `300_000.0`, etc.)
- **`SliceStepper::new()` removed** — identical to `SliceStepper::default()`, kept only
  the derived `Default`
- **`.gitignore` overhaul** — removed stale self-reference, redundant patterns covered
  by `/target`, fixed `mrc` → `*.mrc` pattern
- **16 doc warnings fixed** — intra-doc links in `impl_reader_forwarding!` macro now
  point to public return types instead of `#[doc(hidden)]` trait items

### Linting & Formatting

- **Extended clippy** — added `clippy::cargo` (warn), `missing_docs` (warn), and
  selective `clippy::pedantic` / `clippy::nursery` lints
- **`cargo fmt` applied** — 3 formatting drift fixes across `convert.rs`, `simd/mod.rs`,
  and `reader.rs`

### Testing

- **New unit tests** (9 added) — `VolumeShape::from_header` with negative i32,
  `contains_block` boundary cases, `checked_linear_index` arithmetic + overflow,
  `VoxelBlock` shape mismatch, `is_full_volume` detection
- **New integration tests** (2 added, total 23) — Float16 (Mode 12) roundtrip with
  direct and `convert::<f32>()` paths; `volumes()` iteration over a volume-stack file
- 91 unit tests, 23 integration tests, 38 doc-tests — all pass cleanly across debug,
  release, and --all-features builds

---

## 2026-07-03 — v0.2.6 → v0.3.0

~5,500 lines changed across 31 files. This release marks the API maturing past the
"add everything" phase: conversion is gated through a single `ConvertReader` rather
than a scatter of per-mode shortcut methods, and the public surface is tidied up
with inherent forwarding.

### Breaking API Changes

| Removed | Replacement |
|---------|-------------|
| `reader.slices_f32()` | `reader.convert::<f32>().slices()` |
| `reader.slabs_f32()` | `reader.convert::<f32>().slabs(k)` |
| `reader.read_volume_f32()` | `reader.convert::<f32>().read_volume()` |
| `slices_f32_body!` macro (internal) | `convert_iter` + `RawRegionIter` |
| `ReaderExt` trait (mid-refactor artifact) | inherent methods via `impl_reader_forwarding!` |

### New Features

- **Unified conversion API** — `ConvertReader` gating all mode conversions through
  a single `.convert::<T>()` entry point, with builder-style configuration for
  complex strategy and M0 interpretation
- **Built-in `ndarray` support** — `reader.to_ndarray::<T>()` returns
  `ndarray::Array3<T>` when the `ndarray` feature is enabled
- **Extended header parsers** — typed parsing for CCP4 symmetry records, MRCO
  legacy records, SerialEM records (with `alpha_tilt` field), and Agard records
- **Inherent forwarding** — all `ReaderMethods` / `ConvertMethods` methods are
  available as inherent methods on `Reader` and `MmapReader`; no trait imports
  needed for normal use
- **SIMD expansion** — added u16→f32, u8→f32, f16↔f32, byte-swap (2/4/8 byte),
  and f32 statistics kernels; all with runtime feature detection
- **Criterion benchmarks** — `benches/bench.rs` covering slices, mmap, write,
  stats, and conversion hot paths
- **Integration test suite** — `tests/integration.rs` with ~21 roundtrip tests
  for all modes, compression, subregion reads, and edge cases

### Refinements

- **`#[cold]` on hot-path error branches** — `cold_bounds_error()` helper in
  `encode_block_to_buf` / `write_block_bytes` hints LLVM to sink error branches
- **Removed `TypeId` + `unsafe` pattern** — `update_running_stats_f32()` eliminated;
  `update_header_stats()` now always reads from disk (Writer), mmap (MmapWriter),
  or in-memory buffer (CompressedWriter). This removes the `std::any::TypeId` trick
  and the `unsafe { core::slice::from_raw_parts }` transmute
- **Added `clippy::perf` to deny** alongside `unwrap_used` and `expect_used`

### Doc Overhaul

- **README.md** — Fixed stale roadmap (CCP4/MRCO/SERI/AGAR were marked undone but
  are implemented). Added extended header parsers to the feature list.
- **AGENTS.md** — Synced dep versions to `Cargo.toml`. Updated clippy reference.
  Bumped test counts (~80→~91 unit, clarified doc-test count).
- **APIs.md** — Added disk-read performance note on `Writer::update_header_stats()`.

### Testing

91 unit tests, 21 integration tests, 33 doc-tests, clippy — all pass cleanly across
debug, release, and --all-features builds.

---

### References

- MRC2014 Specification: https://www.ccpem.ac.uk/mrc-format/mrc2014/
