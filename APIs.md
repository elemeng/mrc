# `mrc` API Reference

> MRC-2014 file format library for cryo-EM / cryo-ET.  
> This document describes the **public API surface** — what's available to you as a user of the crate.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Top-Level Functions](#top-level-functions)
3. [Readers](#readers)
   - [`Reader`](#reader) — buffered in-memory reader
   - [`MmapReader`](#mmapreader) — memory-mapped reader (feature `mmap`)
4. [Writers](#writers)
   - [`WriterBuilder` / `Writer`](#writerbuilder--writer) — standard file I/O
   - [`MmapWriter`](#mmapwriter) — memory-mapped writer (feature `mmap`)
   - [`GzipWriter` / `Bzip2Writer`](#compressed-writers) — compressed writers (feature `gzip` / `bzip2`)
5. [Types](#types)
   - [`Header` / `HeaderBuilder`](#header--headerbuilder)
   - [`VolumeShape` / `VoxelBlock`](#volumeshape--voxelblock)
   - [`Mode`, `Voxel`, `FileEndian`](#mode-voxel-fileendian)
   - [Complex and Special Types](#complex-and-special-types)
6. [Iterators](#iterators)
   - [`RegionIter` and Steppers](#regioniter-and-steppers)
7. [Validation](#validation)
8. [FEI Extended Headers](#fei-extended-headers)
9. [Error Types](#error-types)
10. [Conversion Utilities](#conversion-utilities)
11. [Feature Flags](#feature-flags)

---

## Quick Start

```rust
use mrc::{open, create, VoxelBlock, Mode};

// ── Reading (auto-detects gzip/bzip2) ──
let reader = open("protein.mrc")?;

// Each slice is one Z-plane of [nx, ny, 1]
for slice in reader.slices::<f32>() {
    let block = slice?;
    let data: Vec<f32> = block.data;  // nx * ny floats per slice
}

// ── Writing ──
let mut writer = create("output.mrc")
    .shape([512, 512, 256])       // nx, ny, nz
    .mode::<f32>()                // voxel type
    .finish()?;

// Write one slice at a time
writer.write_block(&VoxelBlock::new(
    [0, 0, 0],                // offset [x, y, z]
    [512, 512, 1],            // shape  [sx, sy, sz]
    vec![0.0f32; 512 * 512],  // voxel data
)?)?;

writer.finalize()?;   // rewrites header with final metadata
```

---

## Top-Level Functions

```rust
// Open a file for reading — auto-detects gzip/bzip2 from magic bytes.
pub fn open<P: AsRef<Path>>(path: P) -> Result<Reader, Error>

// Create a new MRC file for writing — returns a WriterBuilder.
pub fn create<P: AsRef<Path>>(path: P) -> WriterBuilder
```

`open` wraps `Reader::open`. `create` wraps `WriterBuilder::new`. These are the idiomatic entry points for most use cases.

```rust
// Decompression safety limit for gzip/bzip2 files (256 GiB).
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: u64 = 274_877_906_944;
```

---

## Readers

### `Reader`

The standard buffered reader. Loads the **entire file** into a `Vec<u8>` on open. All fields and methods are accessed through inherent impls.

| Method | Returns | Description |
|---|---|---|
| `Reader::open(path)` | `Result<Reader>` | Auto-detect compression, open file |
| `Reader::open_plain(path)` | `Result<Reader>` | Force plain (uncompressed) |
| `Reader::open_gzip(path)` | `Result<Reader>` | Force gzip (requires `gzip`); 256 GiB decompression limit |
| `Reader::open_gzip_with_limit(path, max)` | `Result<Reader>` | Force gzip with custom `max_bytes` limit |
| `Reader::open_bzip2(path)` | `Result<Reader>` | Force bzip2 (requires `bzip2`); 256 GiB decompression limit |
| `Reader::open_bzip2_with_limit(path, max)` | `Result<Reader>` | Force bzip2 with custom `max_bytes` limit |
| `Reader::open_permissive(path)` | `Result<(Reader, Vec<String>)>` | Open with lenient header validation; warnings returned separately |
| `Reader::open_gzip_permissive(path)` | `Result<(Reader, Vec<String>)>` | Permissive gzip |
| `Reader::open_bzip2_permissive(path)` | `Result<(Reader, Vec<String>)>` | Permissive bzip2 |
| `reader.shape()` | `VolumeShape` | Volume dimensions `(nx, ny, nz)` |
| `reader.mode()` | `Mode` | Voxel data mode |
| `reader.header()` | `&Header` | Reference to parsed header |
| `reader.endian()` | `FileEndian` | Detected byte order |
| `reader.data_bytes()` | `&[u8]` | Raw voxel data bytes |
| `reader.ext_header_bytes()` | `&[u8]` | Extended header bytes (empty if none) |
| `reader.read_block_bytes(offset, shape)` | `Result<Vec<u8>>` | Read raw bytes for any sub-block |
| `reader.read_block::<T>(offset, shape)` | `Result<VoxelBlock<T>>` | Deprecated, use `subregion` instead |
| `reader.subregion::<T>(offset, shape)` | `Result<VoxelBlock<T>>` | Read and decode typed sub-block at any offset |
| `reader.read_volume::<T>()` | `Result<VoxelBlock<T>>` | Read the entire volume as a single block |
| `reader.read_volume_f32()` | `Result<VoxelBlock<f32>>` | Read entire volume, auto-convert any mode to `f32` |
| `reader.read_volume_u8()` | `Result<VoxelBlock<u8>>` | Read volume as `u8` (Uint16 narrowing or Packed4Bit unpack) |
| `reader.validate_header_stats()` | `Result<()>` | Cross-check header stats vs actual data (1% tolerance) |

**Iterator methods** (all return lazy `RegionIter` or boxed iterators, see [Iterators](#iterators)):

| Method | Returns | Description |
|---|---|---|
| `reader.slices::<T>()` | `RegionIter<T, SliceStepper>` | One Z-plane at a time |
| `reader.slabs::<T>(k)` | `RegionIter<T, SlabStepper>` | `k` contiguous Z-planes |
| `reader.tiles::<T>(shape)` | `RegionIter<T, TileStepper>` | Arbitrary 3D tiles |
| `reader.images::<T>()` | alias for `slices` | Same as `slices` |
| `reader.image_stack::<T>(k)` | alias for `slabs` | Same as `slabs` |
| `reader.planes::<T>()` | alias for `slices` | Same as `slices` |
| `reader.plane_stack::<T>(k)` | alias for `slabs` | Same as `slabs` |
| `reader.volumes::<T>()` | `Result<RegionIter<T, SlabStepper>>` | One sub-volume per step (volume stacks only) |
| `reader.subregion::<T>(offset, shape)` | `Result<VoxelBlock<T>>` | Single block at given offset/shape |
| `reader.slices_f32()` | iterator yielding `VoxelBlock<f32>` | Auto-converts any mode to `f32` (complex → magnitude) |
| `reader.slabs_f32(k)` | iterator yielding `VoxelBlock<f32>` | Same as `slices_f32` but `k` planes at a time |
| `reader.slices_u8()` | iterator yielding `VoxelBlock<u8>` | Mode 6 (Uint16) or Mode 101 (Packed4Bit); narrows/nibble-unpacks to `u8` |
| `reader.slabs_u8(k)` | iterator yielding `VoxelBlock<u8>` | Same as `slices_u8` but `k` planes at a time |
| `reader.slices_mode0(interp)` | iterator yielding `VoxelBlock<f32>` | Mode 0 (Int8) only; signed or unsigned |
| `reader.slabs_mode0(k, interp)` | iterator yielding `VoxelBlock<f32>` | Same as `slices_mode0` but `k` planes at a time |

### `MmapReader`

Memory-mapped reader with **zero-copy** access for native-endian files.
Requires the `mmap` feature.

| Method | Returns | Description |
|---|---|---|
| `MmapReader::open(path)` | `Result<MmapReader>` | Map file read-only |
| `MmapReader::open_permissive(path)` | `Result<(MmapReader, Vec<String>)>` | Permissive mode |
| `reader.shape()` | `VolumeShape` | Volume dimensions |
| `reader.mode()` | `Mode` | Voxel mode |
| `reader.header()` | `&Header` | Parsed header |
| `reader.endian()` | `FileEndian` | Detected byte order |
| `reader.data_bytes()` | `&[u8]` | Raw voxel data (zero-copy) |
| `reader.ext_header_bytes()` | `&[u8]` | Extended header bytes |
| `reader.slab_as::<T>(z, k)` | `Result<&[T]>` | **Zero-copy** typed access into the mmap (requires native endian + matching type) |
| `reader.read_block_bytes(offset, shape)` | `Result<Vec<u8>>` | Read raw bytes for any sub-block |
| `reader.read_block::<T>(offset, shape)` | `Result<VoxelBlock<T>>` | Deprecated, use `subregion` instead |
| `reader.read_volume::<T>()` | `Result<VoxelBlock<T>>` | Read the entire volume as a single block |
| `reader.read_volume_f32()` | `Result<VoxelBlock<f32>>` | Read entire volume, auto-convert any mode to `f32` |
| `reader.read_volume_u8()` | `Result<VoxelBlock<u8>>` | Read volume as `u8` (Uint16 narrowing or Packed4Bit unpack) |
| `reader.validate_header_stats()` | `Result<()>` | Cross-check header stats |

`MmapReader` also has all the same **iterator methods** as `Reader` (`slices`, `slabs`, `tiles`, `slices_f32`, `slabs_f32`, `slices_u8`, `slices_mode0`, `slabs_mode0`, `volumes`, `subregion`, etc.).

**When to use `MmapReader` vs `Reader`:**

| Criterion | `Reader` | `MmapReader` |
|---|---|---|
| File smaller than available RAM | ✅ | ✅ |
| Very large file (> 4 GB) | ⚠️ load time + RAM | ✅ demand-paged |
| True zero-copy typed access | ❌ | ✅ `slab_as` |
| Auto-detects compression | ✅ | ❌ (uncompressed only — open via `Reader::open` for compressed) |
| Available without `mmap` feature | ✅ | ❌ |

---

## Writers

### `WriterBuilder` / `Writer`

The standard writer. Created via `create(path)` or `WriterBuilder::new(path)`.

**Builder methods:**

```rust
let writer = create("out.mrc")
    .shape([nx, ny, nz])          // volume dimensions (also sets mx,my,mz)
    .mode::<f32>()                // voxel type (i8, i16, u16, f32, etc.)
    .cell_lengths(xlen, ylen, zlen) // unit cell in Å
    .cell_angles(alpha, beta, gamma) // cell angles in degrees
    .ispg(1)                      // space group
    .exttyp(*b"CCP4")             // 4-byte extended header type
    .nsymbt(1024)                 // extended header size in bytes
    .origin([0.0, 0.0, 0.0])     // origin coordinates
    .ext_header_bytes(vec![])     // raw extended header bytes (sets nsymbt)
    .finish()?;                   // → Result<Writer>
```

Additional builder methods behind feature flags:
- `.finish_mmap()?` → `MmapWriter` (feature `mmap`)
- `.finish_gzip()?` → `GzipWriter` (feature `gzip`)
- `.finish_bzip2()?` → `Bzip2Writer` (feature `bzip2`)

**Writer methods:**

| Method | Description |
|---|---|
| `writer.shape()` | Volume dimensions |
| `writer.mode()` | Voxel mode |
| `writer.header()` | Mutable access to header (modify before `finalize`) |
| `writer.write_block::<T>(&block)` | Write a typed voxel block. `T` must match file mode |
| `writer.write_u8_block(&block)` | Convenience: write `VoxelBlock<u8>` to a Uint16 file (auto-widens) |
| `writer.write_u4_block(&block)` | Convenience: write `VoxelBlock<u8>` to a Packed4Bit file (auto-packs, values must be 0–15) |
| `writer.write_f16_from_f32(&block)` | Convenience: write `VoxelBlock<f32>` to a Float16 file (feature `f16`) |
| `writer.write_block_parallel::<T>(&block)` | Parallel-encoded write (feature `parallel`; contiguous XY slabs only) |
| `writer.finalize()` | Rewrite header to disk (call when all blocks are written) |
| `writer.update_header_stats()` | Scan all data, compute dmin/dmax/dmean/rms, update header |

### `MmapWriter`

Memory-mapped writer. Created via `WriterBuilder::finish_mmap()`.
Requires `mmap` feature.

Same API as `Writer` (`write_block`, `write_u8_block`, `write_f16_from_f32`,
`write_block_parallel`, `finalize`, `update_header_stats`).

Key difference: `update_header_stats` does not re-read from disk since data is
already in the memory map. `finalize` flushes the mmap instead of seeking.

### Compressed Writers

```rust
// Type aliases
pub type GzipWriter = CompressedWriter<GzipCompressor>;   // feature `gzip`
pub type Bzip2Writer = CompressedWriter<Bzip2Compressor>; // feature `bzip2`
```

Created via `WriterBuilder::finish_gzip()` / `finish_bzip2()`.

**Important:** Compressed writers buffer the **entire file** in memory and compress
only on `finalize`. Not suitable for large volumes that exceed RAM.

Full API: `shape()`, `mode()`, `header()`, `write_block()`, `write_u8_block()`,
`write_f16_from_f32()`, `update_header_stats()` (reads from in-memory buffer),
`finalize()` (takes `self` by value).

---

## Types

### `Header` / `HeaderBuilder`

The 1024-byte MRC-2014 header. Every field is a public `struct` member.

**Fields:**

| Field | Type | Description |
|---|---|---|
| `nx, ny, nz` | `i32` | Volume dimensions (columns, rows, sections) |
| `mode` | `i32` | Data mode (0=Int8, 1=Int16, 2=Float32, etc.) |
| `nxstart, nystart, nzstart` | `i32` | Sub-volume origin in pixels |
| `mx, my, mz` | `i32` | Sampling along X/Y/Z in unit cell |
| `xlen, ylen, zlen` | `f32` | Cell dimensions in Å |
| `alpha, beta, gamma` | `f32` | Cell angles in degrees |
| `mapc, mapr, maps` | `i32` | Column/row/section axis indices (permutation of 1,2,3) |
| `dmin, dmax, dmean` | `f32` | Density statistics |
| `ispg` | `i32` | Space group (0=image, 1-230=crystallographic, 401-630=volume stack) |
| `nsymbt` | `i32` | Extended header size in bytes |
| `extra` | `[u8; 100]` | Extra bytes (bytes 8-11 = EXTTYP, 12-15 = NVERSION) |
| `origin` | `[f32; 3]` | Volume/phase origin |
| `map` | `[u8; 4]` | Must be `b"MAP "`; `b"MAP\0"`, `b"MAPI"`, and all-zero also accepted for compatibility |
| `machst` | `[u8; 4]` | Machine stamp (LE = `0x44 0x44`, BE = `0x11 0x11`) |
| `rms` | `f32` | RMS deviation |
| `nlabl` | `i32` | Number of labels (0-10) |
| `label` | `[u8; 800]` | Ten 80-byte text labels |

**Key methods on `Header`:**

| Method | Returns | Description |
|---|---|---|
| `Header::new()` | `Header` | Default header (Float32, little-endian, NVERSION=20141) |
| `header.data_offset()` | `usize` | Byte offset from file start to voxel data (= 1024 + nsymbt) |
| `header.data_size()` | `Option<usize>` | Size of voxel data block in bytes (respects Packed4Bit) |
| `header.validate()` | `bool` | Quick validity check |
| `header.validate_detailed()` | `Result<(), HeaderValidationError>` | Full structural validation |
| `header.validate_permissive()` | `Result<Vec<String>>` | Lenient validation with warnings |
| `header.decode_from_bytes(bytes)` | `Header` | Parse from raw 1024 bytes (auto endian) |
| `header.decode_from_bytes_with_info(bytes)` | `(Header, Option<&str>)` | Parse with endian fallback diagnostics |
| `header.encode_to_bytes(&mut [u8; 1024])` | `()` | Encode to raw bytes |
| `header.exttyp()` | `[u8; 4]` | Extended header type from `extra[8..12]` |
| `header.set_exttyp(value)` | `()` | Set extended header type |
| `header.nversion()` | `i32` | NVERSION from `extra[12..16]` |
| `header.set_nversion(value)` | `()` | Set NVERSION |
| `header.get_labels()` | `Vec<String>` | Read up to `nlabl` non-empty labels |
| `header.add_label(text)` | `()` | Add a label (FIFO when full) |
| `header.detect_endian()` | `FileEndian` | Detect byte order from MACHST |
| `header.set_file_endian(endian)` | `()` | Set MACHST and re-encode NVERSION |
| `header.is_single_image()` | `bool` | `nz == 1` |
| `header.is_image_stack()` | `bool` | `ispg == 0` |
| `header.is_volume()` | `bool` | Not a stack and not an image stack |
| `header.is_volume_stack()` | `bool` | `ispg` in 401-630 |
| `header.set_image_stack()` | `()` | Set as image stack |
| `header.set_volume()` | `()` | Set as single volume |
| `header.set_volume_stack(mz)` | `()` | Set as volume stack with sub-volume size `mz` |
| `header.voxel_size()` | `[f32; 3]` | Å/pixel = `cella / mxyz` |
| `header.nstart()` | `[i32; 3]` | `[nxstart, nystart, nzstart]` |
| `header.cell_lengths()` | `[f32; 3]` | `[xlen, ylen, zlen]` |
| `header.cell_angles()` | `[f32; 3]` | `[alpha, beta, gamma]` |
| `header.logical_shape()` | `[usize; 4]` | Python-style `(nvolumes, mz, ny, nx)` |

**`HeaderBuilder` methods:**

```rust
HeaderBuilder::new()
    .shape([nx, ny, nz])         // set dimensions + mx,my,mz
    .mode::<f32>()               // set voxel type → mode
    .cell_lengths(x, y, z)       // cell dimensions in Å
    .cell_angles(a, b, g)        // cell angles in degrees
    .ispg(n)                     // space group
    .exttyp(*b"CCP4")            // extended header type
    .nsymbt(n)                   // extended header size
    .origin([x, y, z])           // origin
    .build()?                    // → Result<Header>
```

### `VolumeShape` / `VoxelBlock`

```rust
pub struct VolumeShape {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}
```

| Method | Returns | Description |
|---|---|---|
| `VolumeShape::new(nx, ny, nz)` | `VolumeShape` | New shape |
| `VolumeShape::from_header(&Header)` | `VolumeShape` | From parsed header |
| `shape.total_voxels()` | `Option<usize>` | `nx * ny * nz` (checked) |
| `shape.is_empty()` | `bool` | Any dimension is zero |
| `shape.contains_block(offset, shape)` | `bool` | Check sub-block fits |
| `shape.checked_linear_index(offset)` | `Option<usize>` | Linear index for a position |

```rust
pub struct VoxelBlock<T> {
    pub offset: [usize; 3],   // [x, y, z] start position
    pub shape: [usize; 3],    // [sx, sy, sz] dimensions
    pub data: Vec<T>,         // contiguously stored voxels
}
```

| Method | Returns | Description |
|---|---|---|
| `VoxelBlock::new(offset, shape, data)` | `Result<Self>` | Validates data length matches shape |
| `block.len()` | `usize` | Number of voxels |
| `block.is_empty()` | `bool` | Zero voxels |
| `block.is_full_volume(&VolumeShape)` | `bool` | Covers entire volume from origin |

### `Mode`, `Voxel`, `FileEndian`

```rust
pub enum Mode {
    Int8 = 0,            // signed 8-bit integer
    Int16 = 1,           // signed 16-bit integer
    Float32 = 2,         // 32-bit float
    Int16Complex = 3,    // 2× i16 (real + imaginary)
    Float32Complex = 4,  // 2× f32 (real + imaginary)
    Uint16 = 6,          // unsigned 16-bit integer
    Float16 = 12,        // 16-bit float (requires `f16` feature)
    Packed4Bit = 101,    // 4-bit packed (read/write via nibble unpack/pack)
}
```

| Method | Returns | Description |
|---|---|---|
| `mode.as_i32()` | `i32` | Raw mode constant |
| `Mode::from_i32(n)` | `Option<Mode>` | Parse from integer |
| `mode.byte_size()` | `usize` | Bytes per voxel (1, 2, 4, or 8) |
| `mode.byte_size_for_count(n)` | `usize` | Bytes for n voxels (accounts for Packed4Bit packing) |
| `mode.is_complex()` | `bool` | True for Int16Complex / Float32Complex |
| `mode.is_integer()` | `bool` | True for integer-based modes |
| `mode.is_float()` | `bool` | True for float-based modes |

**`Voxel` trait** — connects Rust types to their MRC mode at compile time.

```rust
pub trait Voxel: EndianCodec + Copy + Send + Sync + Default + 'static {
    const MODE: Mode;
}
```

| Type | Mode |
|---|---|
| `i8` | `Mode::Int8` |
| `i16` | `Mode::Int16` |
| `f32` | `Mode::Float32` |
| `u16` | `Mode::Uint16` |
| `Int16Complex` | `Mode::Int16Complex` |
| `Float32Complex` | `Mode::Float32Complex` |
| `half::f16` (feature `f16`) | `Mode::Float16` |

```rust
pub enum FileEndian {
    LittleEndian,
    BigEndian,
}
```

| Method | Returns | Description |
|---|---|---|
| `FileEndian::from_machst(machst)` | `FileEndian` | Detect from 4-byte MACHST |
| `FileEndian::from_machst_with_info(machst)` | `MachstInfo` | Detect with metadata |
| `endian.to_machst()` | `[u8; 4]` | Standard MACHST bytes |
| `endian.opposite()` | `FileEndian` | The other endianness |
| `FileEndian::native()` | `FileEndian` | Host platform endianness |
| `endian.is_native()` | `bool` | Matches host? |

### Complex and Special Types

```rust
pub struct Int16Complex {
    pub real: i16,
    pub imag: i16,
}

pub struct Float32Complex {
    pub real: f32,
    pub imag: f32,
}
```

Both have `to_real(strategy: ComplexToRealStrategy) -> f32`:
- `RealPart` / `ImaginaryPart` — return the component as `f32`
- `Magnitude` — `sqrt(real² + imag²)`
- `Phase` — `atan2(imag, real)`

Packed4Bit does **not** implement `Voxel` — instead it is handled transparently
by the unified API:
- Reading: [`slices_u8`](Reader::slices_u8) / [`slabs_u8`](Reader::slabs_u8) /
  [`read_volume_u8`](Reader::read_volume_u8) unpack nibbles to `u8` (0–15);
  [`slices_f32`](Reader::slices_f32) / [`read_volume_f32`](Reader::read_volume_f32)
  convert directly to `f32`.
- Writing: [`write_u4_block`](Writer::write_u4_block) packs `u8` values
  two-per-byte.
- Sub-block reads with odd X-offset are rejected (nibble alignment).

```rust
pub enum ComplexToRealStrategy { RealPart, ImaginaryPart, Magnitude, Phase }
pub enum M0Interpretation { Signed, Unsigned }
```

`M0Interpretation` controls how Mode 0 (8-bit) data is read: `Signed` treats raw bytes as `i8`, `Unsigned` as `u8`.

---

## Iterators

### `RegionIter` and Steppers

`RegionIter<T, R, S>` is a **lazy iterator** that yields `Result<VoxelBlock<T>>`.
Created by reader methods like `.slices::<f32>()`.

```rust
pub trait Stepper { /* internal, #[doc(hidden)] */ }

pub struct SliceStepper;    // one Z-plane at a time
pub struct SlabStepper;     // k contiguous Z-planes
pub struct TileStepper;     // arbitrary 3D tiles
```

| Stepper | Construct | Behaviour |
|---|---|---|
| `SliceStepper` | `SliceStepper::new()` (or default) | Steps `z` from 0..nz, block shape = `[nx, ny, 1]` |
| `SlabStepper` | `SlabStepper::new(k)` | Steps `k` slices at a time, block shape = `[nx, ny, k]` |
| `TileStepper` | `TileStepper::new([sx, sy, sz])` | Raster-scans volume in `[sx, sy, sz]` tiles |

You don't normally interact with `RegionIter` directly — just use the reader methods that return them:

```rust
for slice in reader.slices::<f32>() {
    let block = slice?;
    // block.offset, block.shape, block.data
}

for slab in reader.slabs::<i16>(4) { ... }  // 4 planes at a time
for tile in reader.tiles::<u16>([64, 64, 8]) { ... }
```

---

## Validation

```rust
use mrc::validate::{validate_full, ValidationReport, ValidationIssue, Severity};
```

| Function | Returns | Description |
|---|---|---|
| `validate_full(path, permissive)` | `Result<ValidationReport>` | Open file and validate (full I/O) |
| `validate_reader(reader, path, compression, warnings)` | `Result<ValidationReport>` | Validate an already-open reader (no re-open) |

```rust
pub struct ValidationReport {
    pub path: String,
    pub compression: String,  // "plain", "gzip", "bzip2"
    pub nx, ny, nz: i32,
    pub mode: i32,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool;                         // no Error-severity issues
    pub fn by_severity(&self, s: Severity) -> impl Iterator; // filter by severity
}

pub struct ValidationIssue {
    pub severity: Severity,   // Error / Warning / Info
    pub category: &'static str, // "Header", "Statistics", etc.
    pub message: String,
}
```

Validation checks:
1. **Header structure** — dimensions, mode, MAP, ISPG, axis mapping, NVERSION, etc.
2. **File size** — expected vs actual
3. **Endianness** — MACHST stamp, native vs non-native
4. **Statistics** — dmin/dmax/dmean/rms vs actual data (1% tolerance)
5. **Data integrity** — NaN / Inf scan in float modes
6. **Volume info** — type (image, stack, volume, volume stack) and dimensions

---

## FEI Extended Headers

```rust
use mrc::{
    Fei1Metadata, Fei2Metadata,
    FEI1_RECORD_SIZE, FEI2_RECORD_SIZE,
    parse_fei1_records, parse_fei2_records,
};
```

```rust
pub const FEI1_RECORD_SIZE: usize = 768;
pub const FEI2_RECORD_SIZE: usize = 888;

// Parse extended header bytes into typed records
pub fn parse_fei1_records(bytes: &[u8]) -> Option<Vec<Fei1Metadata>>;
pub fn parse_fei2_records(bytes: &[u8]) -> Option<Vec<Fei2Metadata>>;

impl Fei1Metadata {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self>;
}

impl Fei2Metadata {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self>;
}
```

`Fei1Metadata` fields include: `metadata_size`, `metadata_version`, `timestamp`,
`microscope_type`, `ht` (high tension), `dose`, `alpha_tilt`, `beta_tilt`,
`x/y/z_stage`, `tilt_axis_angle`, `pixel_size_x/y`, `defocus`, `magnification`,
`camera_length`, `spot_index`, `camera_name`, `integration_time`, `binning`,
`gain`, `offset`, `dwell_time`, `frame_time`, `start_frame`, `end_frame`, etc.

`Fei2Metadata` extends `Fei1Metadata` with: `scan_rotation`, `diffraction_pattern_rotation`,
`image_rotation`, `scan_mode_enumeration`, `acquisition_time_stamp`,
`detector_commercial_name`, `start/end_tilt_angle`, `tilt_per_image`, `tilt_speed`,
`beam_center_x/y_pixel`, `cfeg_flash_timestamp`, `phase_plate_position_index`,
`objective_aperture_name`.

---

## Error Types

### `Error` — top-level error enum

| Variant | When it occurs |
|---|---|
| `Io(std::io::Error)` | Underlying I/O failure |
| `InvalidHeader` | Malformed header |
| `UnsupportedMode` | Mode not recognised |
| `BoundsError` | Block outside volume bounds |
| `TypeMismatch { expected, actual }` | Byte size mismatch |
| `BlockShapeMismatch { expected, actual }` | Data length ≠ block volume |
| `ModeMismatch { file_mode, requested_mode }` | Requested type ≠ file mode |
| `InvalidHeaderDetailed(HeaderValidationError)` | Specific validation failure |
| `StatsMismatch { claimed_*, actual_* }` | Header stats don't match data |
| `Mmap` (feature `mmap`) | Memory mapping failed |
| `FileSizeMismatch { expected, actual }` | Wrong file size |
| `NotAVolumeStack { ispg, mz }` | `volumes()` on non-stack file |

### `HeaderValidationError` — detailed header issues

`InvalidDimensions`, `UnsupportedMode(i32)`, `InvalidMap([u8;4])`, `InvalidIspg(i32)`,
`InvalidAxisMapping { mapc, mapr, maps }`, `InvalidNsymbt(i32)`, `InvalidNlabl(i32)`,
`InvalidNversion(i32)`, `InvalidVolumeStack { nz, mz, ispg }`,
`InvalidSampling { mx, my, mz }`, `LabelCountMismatch { nlabl, actual }`,
`EmptyLabelBeforeFilled { index }`.

---

## Conversion Utilities

```rust
// Reinterpret Mode 0 (8-bit) as signed or unsigned f32
pub fn reinterpret_m0(data: &[u8], interp: M0Interpretation) -> Vec<f32>;

// Widen u8 → u16 for writing as Mode 6
pub fn convert_u8_slice_to_u16(src: &[u8]) -> Vec<u16>;

// Narrow u16 → u8 (returns Err if any value > 255)
pub fn convert_u16_slice_to_u8(src: &[u16]) -> Result<Vec<u8>, Error>;
```

These are convenience functions exposed from the crate root. The more comprehensive
conversion infrastructure (i16→f32, u16→f32, i8→f32) is used internally by
`slices_f32` / `slabs_f32` but not directly exposed.

---

## Feature Flags

| Feature | Default | What it enables |
|---|---|---|
| `mmap` | ✅ | `MmapReader`, `MmapWriter`, `WriterBuilder::finish_mmap()` |
| `f16` | ✅ | `half::f16` type, `Mode::Float16`, `write_f16_from_f32()` |
| `simd` | ✅ | AVX2/NEON accelerated i8/i16/u16→f32 conversions |
| `parallel` | ✅ | `write_block_parallel()` using `rayon` |
| `gzip` | ✅ | Gzip auto-detection, `Reader::open_gzip()`, `GzipWriter` |
| `bzip2` | ❌ | Bzip2 auto-detection, `Reader::open_bzip2()`, `Bzip2Writer` |

---

## Design Notes

**New files are always little-endian.** The crate defaults to LE with NVERSION=20141.
Reading handles both endiannesses transparently.

**Permissive mode** enables lenient header parsing for legacy / non-standard files.
Non-critical issues become warnings instead of errors.

**Compression is transparent on read** — `open()` auto-detects gzip/bzip2 from
magic bytes and decompresses the whole file into memory. A hard cap of
[`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB) prevents decompression bombs.
Use `open_gzip_with_limit()` / `open_bzip2_with_limit()` for a custom limit.

[`DEFAULT_MAX_DECOMPRESSED_BYTES`]: #top-level-functions

**`finalize()` rewrites the header** — the header is written optimistically at
file creation and rewritten at the end to capture any modifications (e.g. updated
stats, labels). Every MRC file should call `finalize()`.
