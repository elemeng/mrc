# mrc

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/mrc.svg)](https://crates.io/crates/mrc)
[![Docs.rs](https://img.shields.io/docsrs/mrc.svg)](https://docs.rs/mrc)

> **Type-safe MRC-2014 file format reader/writer for Rust**

A high-performance, memory-efficient library for reading and writing MRC (Medical Research Council) format files used in cryo-electron microscopy and structural biology.

## Installation

```toml
[dependencies]
mrc = "0.5"
```

## Quick Start

```rust,no_run
use mrc::{open, create, VoxelBlock};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// Read (auto-detects compression; handles common microscope quirks
// like NVERSION=0 and "MAP\0" automatically)
let reader = open("protein.mrc")?;
for slice in reader.convert::<f32>().slices() {
    let block = slice?;  // Vec<f32>
}

// Write — **always call .finalize()** or the header will be stale
// (density statistics wrong, tools display garbage contrast)
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;
writer.write_block(&VoxelBlock::new(
    [0, 0, 0], [512, 512, 1], vec![0.0f32; 512 * 512],
)?)?;
writer.update_header_stats()?;  // compute density statistics
writer.finalize()?;             // rewrite header with final metadata
# Ok(()) }
```

## Full Documentation

See **[docs.rs/mrc](https://docs.rs/mrc)** for the complete API reference, or
**[APIs.md](APIs.md)** in this repository for a local API surface overview.

- Reading files — `Reader` (auto-selects mmap or buffered), compressed I/O, permissive mode,
  decompression bomb protection (256 GiB limit, configurable);
  also `from_reader()` / `from_bytes()` for in-memory and stream sources
- Writing files — `Writer`; `from_writer()` for in-memory targets; `Compression` level control
- Iterators — slices, slabs, tiles, volumes
- Auto-conversion — `reader.convert::<T>().slices()` etc.
- Full-volume reads — `read_volume::<T>()`, `read_volume_u8()`,
  auto-conversion via `reader.convert::<T>().read_volume()`
- Data modes — `Mode` enum and compile-time `Voxel` trait, including Packed4Bit
  read/write via `slices_u8` / `write_u4_block`
- Headers — `Header`, `HeaderBuilder`, validation, endianness
- Extended header parsers — FEI1/FEI2 metadata, CCP4 symmetry records,
  MRCO legacy records, SerialEM records, Agard records
- Error handling — `Error` and `HeaderValidationError`
- Validation — `validate_full` / `validate_reader` / `ValidationReport`
- **Real-world workflows** — tilt series, FEI metadata, volume stacks
- **Troubleshooting** — common errors and how to fix them

## CLI Tools

| Binary | Description |
|--------|-------------|
| `mrc-validate` | Comprehensive file validation (header, stats, NaN/Inf scan) |
| `mrc-header`   | Human-readable header inspection with semantic interpretation |
| `mrc-invert`   | Contrast inversion (v → −v) with updated header statistics |

```bash
cargo build --release --bin mrc-validate
./mrc-validate protein.mrc
```

## Version History

See [update.md](update.md) for detailed changelogs covering all releases from v0.2.6 onward.

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

## License

MIT — see the LICENSE file.

## Acknowledgments

- [CCP-EM](https://www.ccpem.ac.uk/) for the MRC-2014 specification
- EMDB for providing real-world test data
- Cryo-EM community for invaluable feedback
