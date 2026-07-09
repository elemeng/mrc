# Road Map

See [update.md](update.md) for detailed changelogs covering all releases from v0.2.6 onward.
Also see the [README](README.md) for a quick-start guide and feature overview.

**v0.3.x** — Stabilization & Quality ✅

- [x] Complete MRC-2014 format support
- [x] Iterator-centric API (slices, slabs, tiles)
- [x] Type-safe I/O with compile-time mode checking
- [x] SIMD acceleration (AVX2, NEON) — i8↔f32, i16↔f32, u16↔f32, f16↔f32, byte-swap, stats, f32→i16/u16/i8
- [x] Memory-mapped I/O and parallel encoding
- [x] All data types (modes 0–4, 6, 12, 101)
- [x] Compression support (gzip, bzip2)
- [x] All extended header parsers (FEI1/2, CCP4, MRCO, SERI, AGAR)
- [x] Header statistics computation and validation
- [x] Permissive mode and volume stack support
- [x] Decompression bomb protection (configurable 256 GiB limit)
- [x] Criterion benchmark suite + integration tests
- [x] Unified `ConvertReader` API with inherent forwarding
- [x] `ndarray` feature for numpy-like volume access
- [x] SIMD f32→i16/i8 clamping in write-hot paths
- [x] Richer error context (offset, mode in BoundsError / ModeMismatch)

**v0.4.x** — Quality, Header API & Polish ✅

- [x] Optional serde support (`serde` feature) for public types
- [x] `tracing` facade replacing `eprintln!` (library diagnostics)
- [x] Auto-dispatch extended header parsing — `reader.parse_extended_header()`
- [x] Reader convenience methods — `reader.fei1_metadata()`, `reader.ccp4_records()`, etc.
- [x] Expand IMOD metadata with more fields from `extra` bytes
- [x] Richer `Header` convenience API — `cell_volume()`, `label_at()`, `density_stats()`, etc.
- [x] `ExtHeaderType` + `ExtHeaderData` dispatch enum
- [x] Miri CI job in GitHub Actions
- [x] Extended clippy linting (`cargo`, `missing_docs`)
- [x] Richer error context (offset, mode) in BoundsError / ModeMismatch
- [x] `#[must_use]` audit on builder and accessor methods

**v0.5.x** — Consolidation & Polish ✅

- [x] Fixed `is_truncated()` for buffered readers (previously always returned `false`)
- [x] Added `Writer::header_mut()` for mutable header access mid-write
- [x] Added missing builder setters (`cell_angles`, `nstart`, `sampling`, `axis_mapping`, `add_label`, `mode_raw`)
- [x] Eliminated O(n) f32 clone in `write_block_as_body` via `write_block_data()` extraction
- [x] Fixed `write_u4_block` returning `BoundsError` instead of `ValueOutOfRange`
- [x] Corrected `write_u8_block` to avoid unnecessary `VoxelBlock` construction
- [x] Added `#[must_use]` to `WriterBuilder::ext_header_bytes`
- [x] Comprehensive documentation audit across all doc files
- [x] Restructured and enriched crate-level docs.rs documentation

**v0.6.x** — Robust real-world testing across all public APIs

- [ ] Test every public API item with real MRC files in every mode
- [ ] Cover all read/write/convert/validate/header paths with actual cryo-EM data
- [ ] Ensure edge cases (truncated, compressed, permissive, volume stacks, extended headers) are exercised with real files

**Note:** This crate is under active development. While most features are functional, occasional API changes are possible. Contributions welcome — please report issues and share your ideas!
