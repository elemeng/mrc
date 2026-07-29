# `mrc` API Reference

> MRC-2014 file format library for cryo-EM / cryo-ET.  
> This document describes the **public API surface** — what's available to you as a user of the crate.

---

## Table of Contents

### Basic API
1. [Quick Start](#basic-quick-start)
2. [Top-Level Functions](#basic-top-level-functions)
3. [Reading Data](#basic-reading-data)
4. [Writing Data](#basic-writing-data)
5. [Data Modes](#basic-data-modes)
6. [Compression Auto-Detection](#basic-compression)
7. [Reading from Memory / Streams](#basic-memory-streams)
8. [Troubleshooting](#basic-troubleshooting)

### Advanced API
9. [Raw Byte Access](#advanced-raw-bytes)
10. [Permissive Mode](#advanced-permissive)
11. [Special-Mode Reads](#advanced-special-modes)
12. [Headers](#advanced-headers)
13. [Extended Headers](#advanced-extended-headers)
14. [Validation](#advanced-validation)
15. [Error Types](#advanced-errors)
16. [Conversion Utilities](#advanced-conversion)
17. [Feature Flags](#advanced-features)
18. [Types](#advanced-types)
19. [Design Notes](#advanced-design)

---

## Basic API

These are the APIs you'll use every day.  They return typed data (zero-copy borrowed
views) and auto-detect compression, endianness, and the file's voxel mode.

### <a name="basic-quick-start"></a>Quick Start

```rust
use mrc::{read_as, write_as, open, create, VoxelBlock};

// ── One-shot read (open + read_volume) ──
let (header, data): (_, Vec<f32>) = read_as("protein.mrc")?;
println!("{}×{}×{}, mode {}", header.nx, header.ny, header.nz, header.mode);

// ── One-shot write (create + write_block + finalize) ──
write_as("output.mrc", &data, [512, 512, 256])?;

// ── Iterative read (streaming, memory-friendly) ──
let reader = open("protein.mrc")?;
for slice in reader.slices() {
    let block = slice?;
    match block.data() {
        mrc::DataView::Float32(data) => { /* process &[f32] */ }
        _ => {}
    }
}

// ── Streaming write (one slice at a time) ──
let mut writer = create("output.mrc")
    .shape([512, 512, 256])
    .mode::<f32>()
    .finish()?;
let block = DataBlock::Owned {
    offset: [0, 0, 0],
    shape: [512, 512, 1],
    data: OwnedData::Float32(vec![0.0f32; 512 * 512]),
};
writer.write_data_block(&block)?;
writer.finalize()?;
```

---

### <a name="basic-top-level-functions"></a>Top-Level Functions

```rust
// Open a file for reading — auto-detects gzip/bzip2 from magic bytes.
pub fn open<P: AsRef<Path>>(path: P) -> Result<Reader, Error>

// Create a new MRC file for writing — returns a WriterBuilder.
pub fn create<P: AsRef<Path>>(path: P) -> WriterBuilder

// One-shot read: open + read_volume, returns (Header, Vec<T>).
pub fn read_as<T: Voxel, P: AsRef<Path>>(path: P) -> Result<(Header, Vec<T>), Error>

// One-shot write: create + set_data + finalize, single call.
pub fn write_as<T: Voxel, P: AsRef<Path>>(path: P, data: &[T], shape: [usize; 3]) -> Result<()>
```

`open` wraps `Reader::open`. `create` wraps `WriterBuilder::new`.

```rust
// Decompression safety limit for gzip/bzip2 files (256 GiB).
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: u64 = 274_877_906_944;
```

---

### <a name="basic-reading-data"></a>Reading Data

Open any MRC file with `Reader::open` — compression, byte order, and voxel type
are detected automatically.

| Constructor | Returns | Description |
|---|---|---|
| `Reader::open(path)` | `Result<Reader>` | Auto-detect compression, open file |
| `Reader::open_plain(path)` | `Result<Reader>` | Force plain (uncompressed) |

**Iteration methods** — each returns `DataBlock<'_>` whose `DataView` variant
is determined by the file's mode at runtime:

| Method | Returns | Description |
|---|---|---|
| `reader.subregion(offset, shape)` | `Result<DataBlock<'_>>` | Single block by coordinate |
| `reader.read_volume()` | `Result<DataBlock<'_>>` | Entire volume |
| `reader.slices()` | `impl Iterator<Item = Result<DataBlock<'_>>>` | One Z-plane at a time |
| `reader.slabs(k)` | `impl Iterator<Item = Result<DataBlock<'_>>>` | `k` contiguous Z-planes |
| `reader.tiles(shape)` | `impl Iterator<Item = Result<DataBlock<'_>>>` | Arbitrary 3D tiles |
| `reader.volumes()` | `Result<impl Iterator<...>>` | Sub-volumes (stacks only) |

Use `reader.convert::<T>()` to auto-convert any mode to `f32` (or `i16`, `u16`, etc.):

```rust
for slice in reader.convert::<f32>().slices() {
    let block = slice?;
    // block.data: Vec<f32>
}
```

`ConvertReader` methods: `slices()`, `slabs(k)`, `tiles(shape)`, `volumes()`,
`subregion(offset, shape)`, `read_volume()`, `to_ndarray()` (feature `ndarray`).
Configure complex reduction with `.with_complex_strategy(s)` and M0
interpretation with `.with_m0_interpretation(i)`.

**Reader metadata:**

| Method | Returns | Description |
|---|---|---|
| `reader.shape()` | `VolumeShape` | Dimensions `(nx, ny, nz)` |
| `reader.mode()` | `Mode` | Voxel data mode |
| `reader.header()` | `&Header` | Reference to parsed header |
| `reader.endian()` | `FileEndian` | Detected byte order |
| `reader.is_single_image()` | `bool` | `nz == 1` |
| `reader.is_image_stack()` | `bool` | `ispg == 0` |
| `reader.is_volume()` | `bool` | Not a stack nor image stack |
| `reader.is_volume_stack()` | `bool` | `ispg` in 401–630 |
| `reader.logical_shape()` | `[usize; 4]` | `[nvolumes, mz, ny, nx]` |
| `reader.is_truncated()` | `bool` | True if permissive file is short |

---

### <a name="basic-writing-data"></a>Writing Data

Created via `create(path)` or `WriterBuilder::new(path)`.

**Builder methods:**

```rust
let writer = create("out.mrc")
    .shape([nx, ny, nz])            // volume dimensions
    .mode::<f32>()                  // voxel type
    .mode_raw(101)                  // or set raw mode (no Voxel impl)
    .cell_lengths(xlen, ylen, zlen) // unit cell in Å
    .ispg(1)                        // space group
    .origin([0.0, 0.0, 0.0])       // origin coordinates
    .volume_stack(30)              // volume stack (ispg=401, mz=30)
    .image_stack()                 // image stack (ispg=0, mz=1)
    .volume()                      // single volume (ispg=1, mz=nz)
    .finish()?;                    // open the file
```

Additional backends: `.finish_buffer()?` (in-memory), `.finish_mmap()?`,
`.finish_gzip()?`, `.finish_bzip2()?`.

**Writer methods:**

| Method | Description |
|---|---|
| `writer.write_data_block(&DataBlock)` | Write a block with runtime mode dispatch; each call encodes and writes immediately (streaming, no buffer) |
| `writer.write_block_as(&VoxelBlock<T>)` | Write with auto-conversion to file's mode |
| `writer.write_u8_block(&block)` | Write `u8` data to Uint16 file (auto-widens) |
| `writer.write_u4_block(&block)` | Write `u8` data to Packed4Bit file (auto-packs) |
| `writer.set_data(&data)` | Write full volume + compute stats |
| `writer.update_header_stats()` | Scan data, update dmin/dmax/dmean/rms |
| `writer.header()` | Read-only header reference |
| `writer.header_mut()` | Mutable header reference |
| `writer.finalize()` | **Required** — rewrites header with final metadata |

---

### <a name="basic-data-modes"></a>Data Modes

| Mode | Rust type | Typical use |
|---|---|---|
| `Int8` (0) | `i8` | Binary masks |
| `Int16` (1) | `i16` | Raw cryo-EM density |
| `Float32` (2) | `f32` | Processed / reconstructed density |
| `Int16Complex` (3) | `Int16Complex` | Complex (i16 real + i16 imag) |
| `Float32Complex` (4) | `Float32Complex` | Complex (f32 real + f32 imag) |
| `Uint16` (6) | `u16` | Segmentation labels |
| `Float16` (12) | `f16` | Half-precision (feature `f16`) |
| `Packed4Bit` (101) | `u8` via `slices_u8` | 4-bit packed data |

Use `reader.convert::<f32>()` to read any mode as `f32`.

---

### <a name="basic-compression"></a>Compression Auto-Detection

| Magic bytes | Format |
|---|---|
| `\x1f\x8b` | Gzip |
| `BZ` | Bzip2 |
| anything else | Plain |

Plain files use mmap (zero-copy) or buffered I/O.  Compressed files decompress
into memory (capped at 256 GiB).

| Constructor | Description |
|---|---|
| `Reader::open_gzip(path)` | Force gzip |
| `Reader::open_gzip_with_limit(path, max)` | Gzip with custom limit |
| `Reader::open_bzip2(path)` | Force bzip2 (feature `bzip2`) |
| `Reader::open_bzip2_with_limit(path, max)` | Bzip2 with custom limit |

---

### <a name="basic-memory-streams"></a>Reading from Memory / Streams

When data is already in memory (e.g. from a camera readout or network
stream), use `Reader::from_reader` or `Reader::from_bytes`:

```rust
use mrc::Reader;
use std::io::Cursor;

let bytes = std::fs::read("density.mrc")?;
let reader = Reader::from_reader(Cursor::new(bytes))?;
```

| Method | Returns | Description |
|---|---|---|
| `Reader::from_reader(r)` | `Result<Reader>` | Read from any `Read` source |
| `Reader::from_bytes(data)` | `Result<Reader>` | Parse from `Vec<u8>` |
| `Reader::from_reader_permissive(r)` | `Result<(Reader, Vec<String>)>` | Permissive |
| `Reader::from_bytes_permissive(data)` | `Result<(Reader, Vec<String>)>` | Permissive |

---

### <a name="basic-troubleshooting"></a>Troubleshooting

| Error | Likely cause | What to try |
|---|---|---|
| `InvalidHeader` | Not an MRC file | `mrc validate file.mrc` or try `open_permissive` |
| `FileSizeMismatch` | Truncated or trailing garbage | Re-download or run `mrc validate` |
| `ModeMismatch` | Block type != file mode | Use `write_block_as` for auto-conversion |
| `BoundsError` | Block outside volume | Check offset + shape ≤ dimensions |
| `UnsupportedMode` | Mode needs `f16` feature | Enable `f16` or convert with another tool |

---

## Advanced API

These APIs give you lower-level access to raw bytes, headers, extended metadata,
and validation.  They are intended for tools, pipelines, and developers who need
more control than the basic iterators provide.

### <a name="advanced-raw-bytes"></a>Raw Byte Access

Three methods expose on-disk bytes directly:

⚠️ **The returned bytes are raw on-disk bytes** — file byte order,
no endian correction, no type conversion.  Use `reader.endian()` and
`reader.mode()` to interpret them correctly.  For typed zero-copy
access, use `subregion`/`slices` or `convert::<f32>()`.

| Method | Returns | Copy cost |
|---|---|---|
| `reader.raw_bytes()` | `&[u8]` — whole data region | zero-copy |
| `reader.read_block_bytes_cow(offset, shape)` | `Cow<[u8]>` — sub-block | zero-copy for contiguous XY slabs |
| `reader.read_block_bytes(offset, shape)` | `Vec<u8>` — sub-block | **always copies** |

**Contiguous** = offset `[0, y, z]`, shape `[nx, ny, sz]`.  Any sub-XY offset
or shape forces a row-by-row gather into owned memory.

⚠️ `read_block_bytes` always returns an owned `Vec`, even for contiguous blocks.
For large volumes this causes a full copy.  Use `read_block_bytes_cow` to avoid
the allocation, or use `subregion`/`slices` for typed zero-copy access.

```rust
use std::borrow::Cow;

// Zero-copy: borrows from mmap for contiguous blocks
let cow: Cow<[u8]> = reader.read_block_bytes_cow([0, 0, 0], [256, 256, 64])?;

// Always owned: allocates + copies
let bytes: Vec<u8> = reader.read_block_bytes([0, 0, 0], [256, 256, 64])?;
```

Other raw access:

| Method | Returns | Description |
|---|---|---|
| `reader.raw_bytes()` | `&[u8]` | All voxel data as on-disk bytes |
| `reader.ext_header_bytes()` | `&[u8]` | Extended header bytes |
| `reader.validate_header_stats()` | `Result<()>` | Cross-check header stats vs data |

---

### <a name="advanced-permissive"></a>Permissive Mode

Turns non-critical header issues into warnings instead of errors.

| Method | Description |
|---|---|
| `Reader::open_permissive(path)` | Open with lenient header validation |
| `Reader::open_gzip_permissive(path)` | Permissive gzip |
| `Reader::open_bzip2_permissive(path)` | Permissive bzip2 |

---

### <a name="advanced-special-modes"></a>Special-Mode Reads (Mode 0, Packed4Bit)

| Method | Returns | Description |
|---|---|---|
| `reader.slices_u8()` | iterator | Unpack Packed4Bit / narrow Uint16 to `u8` |
| `reader.slabs_u8(k)` | iterator | Same but `k` planes at a time |
| `reader.read_volume_u8()` | `VoxelBlock<u8>` | Full Packed4Bit volume as `u8` |
| `reader.slices_mode0(interp)` | iterator | Mode 0 as `f32` (signed/unsigned) |
| `reader.slabs_mode0(k, interp)` | iterator | Same but `k` planes at a time |

---

### <a name="advanced-headers"></a>Headers

The 1024-byte MRC-2014 header.  Every field is a public `struct` member.

**Fields:**

| Field | Type | Description |
|---|---|---|
| `nx, ny, nz` | `i32` | Volume dimensions |
| `mode` | `i32` | Data mode |
| `nxstart, nystart, nzstart` | `i32` | Sub-volume origin in pixels |
| `mx, my, mz` | `i32` | Cell sampling |
| `xlen, ylen, zlen` | `f32` | Cell dimensions in Å |
| `alpha, beta, gamma` | `f32` | Cell angles |
| `mapc, mapr, maps` | `i32` | Axis mapping |
| `dmin, dmax, dmean` | `f32` | Density statistics |
| `ispg` | `i32` | Space group |
| `nsymbt` | `i32` | Extended header size |
| `extra` | `[u8; 100]` | Extra bytes (EXTTYP, NVERSION) |
| `origin` | `[f32; 3]` | Volume/phase origin |
| `map` | `[u8; 4]` | Must be `b"MAP "` |
| `machst` | `[u8; 4]` | Machine stamp |
| `rms` | `f32` | RMS deviation |
| `nlabl` | `i32` | Number of labels |
| `label` | `[u8; 800]` | Ten 80-byte labels |

**Key methods:**

| Method | Returns | Description |
|---|---|---|
| `Header::new()` | `Header` | Default header |
| `header.data_offset()` / `.data_size()` | `usize` / `Option<usize>` | Data region location |
| `header.validate()` / `.validate_detailed()` / `.validate_permissive()` | varies | Validation |
| `header.decode_from_bytes(bytes)` | `Header` | Parse raw 1024 bytes |
| `header.encode_to_bytes(&mut [u8; 1024])` | `()` | Encode to bytes |
| `header.exttyp()` / `.exttyp_str()` / `.set_exttyp(v)` | varies | Extended header type |
| `header.nversion()` / `.set_nversion(v)` | `i32` / `()` | Version |
| `header.get_labels()` / `.add_label(t)` / `.label_at(i)` | varies | Text labels |
| `header.detect_endian()` / `.set_file_endian(e)` | `FileEndian` / `()` | Byte order |
| `header.voxel_size()` / `.sampling()` | `[f32; 3]` / `[i32; 3]` | Spatial resolution |
| `header.density_stats()` | `(f32, f32, f32, f32)` | `(dmin, dmax, dmean, rms)` |
| `header.cell_lengths()` / `.cell_angles()` / `.cell_volume()` | varies | Unit cell |
| `header.logical_shape()` | `[usize; 4]` | `[nvolumes, mz, ny, nx]` |
| `header.is_single_image()` / `.is_image_stack()` / `.is_volume()` / `.is_volume_stack()` | `bool` | Volume type |
| `header.set_volume_stack(mz)` / `.set_image_stack()` / `.set_volume()` | `()` | Configure type |
| `header.detect_imod()` / `.is_y_inverted()` | `Option<ImodInfo>` / `bool` | IMOD metadata |
| `header.is_standard_map()` | `bool` | MAP is exactly `"MAP "` |
| `header.nstart()` | `[i32; 3]` | `[nxstart, nystart, nzstart]` |

**`HeaderBuilder` methods:**

```rust
HeaderBuilder::new()
    .shape([nx, ny, nz])         // dimensions + mx,my,mz
    .mode::<f32>()               // voxel type → mode
    .mode_raw(101)               // raw mode (no Voxel impl)
    .cell_lengths(x, y, z)       // cell in Å
    .cell_angles(a, b, g)        // cell angles
    .ispg(n)                     // space group
    .exttyp(*b"CCP4")            // extended header type
    .nsymbt(n)                   // extended header size
    .origin([x, y, z])           // origin
    .nstart([x, y, z])           // sub-volume origin
    .sampling([mx, my, mz])      // cell sampling
    .axis_mapping([1, 2, 3])     // column/row/section mapping
    .add_label("my volume")      // text label
    .set_volume_stack(30)        // volume stack
    .build()?                    // → Result<Header>
```

---

### <a name="advanced-extended-headers"></a>Extended Headers

| Format | Record size | Typical use |
|---|---|---|
| CCP4 | 80 bytes | CCP4 symmetry records |
| MRCO | 80 bytes | Legacy MRC format |
| SERI | 256 bytes | SerialEM tilt-series |
| AGAR | 1024 bytes | Agard metadata |
| FEI1 | 768 bytes | FEI microscope metadata |
| FEI2 | 888 bytes | FEI extended metadata |

Use `reader.parse_extended_header()` for auto-detection.

Convenience methods: `reader.fei1_metadata()`, `reader.fei2_metadata()`,
`reader.ccp4_records()`, `reader.mrco_records()`, `reader.seri_records()`,
`reader.agar_records()`, `reader.imod_metadata()`.

---

### <a name="advanced-validation"></a>Validation

| Function | Returns | Description |
|---|---|---|
| `validate_full(path, permissive)` | `Result<ValidationReport>` | Open + validate |
| `validate_reader(reader, path, compression, warnings)` | `Result<ValidationReport>` | Validate open reader |

```rust
pub struct ValidationReport {
    pub path: String,
    pub compression: String,
    pub nx: i32, pub ny: i32, pub nz: i32, pub mode: i32,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool;
    pub fn by_severity(&self, s: Severity) -> impl Iterator;
}
```

Checks: header structure, file size, endianness, statistics (1% tolerance),
NaN/Inf scan, volume type.

---

### <a name="advanced-errors"></a>Error Types

**`Error`** — top-level enum.  Variants:

`Io`, `InvalidHeader`, `UnsupportedMode`, `BoundsError`,
`TypeMismatch`, `ValueOutOfRange`, `BlockShapeMismatch`, `ModeMismatch`,
`InvalidHeaderDetailed(HeaderValidationError)`, `StatsMismatch`, `Mmap`
(feature `mmap`), `FileSizeMismatch`, `NotAVolumeStack`.

**`HeaderValidationError`** — fine-grained header diagnostics:

`InvalidDimensions`, `UnsupportedMode(i32)`, `InvalidMap([u8;4])`,
`InvalidIspg(i32)`, `InvalidAxisMapping`, `InvalidNsymbt(i32)`,
`InvalidNlabl(i32)`, `InvalidNversion(i32)`, `InvalidVolumeStack`,
`InvalidSampling`, `LabelCountMismatch`, `EmptyLabelBeforeFilled`.

---

### <a name="advanced-conversion"></a>Conversion Utilities

```rust
pub fn reinterpret_m0(data: &[u8], interp: M0Interpretation) -> Vec<f32>;
pub fn convert_u8_slice_to_u16(src: &[u8]) -> Vec<u16>;
pub fn convert_u16_slice_to_u8(src: &[u16]) -> Result<Vec<u8>, Error>;
```

---

### <a name="advanced-features"></a>Feature Flags

| Feature | Default | What it enables |
|---|---|---|
| `mmap` | ✅ | Memory-mapped I/O |
| `f16` | ✅ | `half::f16`, Mode 12 |
| `simd` | ✅ | AVX2/NEON acceleration |
| `parallel` | ✅ | Parallel decode/convert/encode via rayon |
| `gzip` | ✅ | Gzip auto-detection + compressed writer |
| `bzip2` | ❌ | Bzip2 auto-detection + compressed writer |
| `ndarray` | ❌ | Return volumes as `Array3<T>` |
| `serde` | ❌ | Serialize/Deserialize support |

---

### <a name="advanced-types"></a>Types

**`VolumeShape`:**

```rust
pub struct VolumeShape { pub nx: usize, pub ny: usize, pub nz: usize }
```

Methods: `new`, `from_header`, `total_voxels`, `is_empty`, `contains_block`,
`checked_linear_index`.

**`VoxelBlock<T>`:**

```rust
pub struct VoxelBlock<T> {
    pub offset: [usize; 3],
    pub shape: [usize; 3],
    pub data: Vec<T>,
}
```

Methods: `new`, `len`, `is_empty`, `is_full_volume`.

**`DataBlock<'a>`** — returned by default reader methods:

```rust
pub enum DataBlock<'a> {
    Borrowed { offset: [usize; 3], shape: [usize; 3], data: DataView<'a> },
    Owned { offset: [usize; 3], shape: [usize; 3], data: OwnedData },
}
```

Methods: `offset()`, `shape()`, `data()` → `DataView<'_>`.

**`DataView<'a>`:**

```rust
pub enum DataView<'a> {
    Int8(&'a [i8]), Int16(&'a [i16]), Float32(&'a [f32]),
    Int16Complex(&'a [Int16Complex]), Float32Complex(&'a [Float32Complex]),
    Uint16(&'a [u16]), Float16(&'a [half::f16]), Packed4Bit(&'a [u8]),
}
```

**`Mode`:**

| Method | Returns | Description |
|---|---|---|
| `mode.as_i32()` | `i32` | Raw constant |
| `Mode::from_i32(n)` | `Option<Mode>` | Parse from integer |
| `mode.byte_size()` | `usize` | Bytes per voxel |
| `mode.byte_size_for_count(n)` | `usize` | Bytes for n voxels |
| `mode.is_complex()` / `.is_integer()` / `.is_float()` | `bool` | Type category |

**`Voxel` trait:**

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

**`FileEndian`:**

| Method | Returns |
|---|---|
| `FileEndian::from_machst(machst)` | `FileEndian` |
| `FileEndian::from_machst_with_info(machst)` | `MachstInfo` |
| `endian.to_machst()` | `[u8; 4]` |
| `endian.opposite()` | `FileEndian` |
| `FileEndian::native()` | `FileEndian` |
| `endian.is_native()` | `bool` |

**Complex types:**

```rust
pub struct Int16Complex { pub real: i16, pub imag: i16 }
pub struct Float32Complex { pub real: f32, pub imag: f32 }
```

Both have `to_real(strategy: ComplexToRealStrategy) -> f32`:
`RealPart`, `ImaginaryPart`, `Magnitude`, `Phase`.

```rust
pub enum ComplexToRealStrategy { RealPart, ImaginaryPart, Magnitude, Phase }
pub enum M0Interpretation { Signed, Unsigned }
```

---

### <a name="advanced-design"></a>Design Notes

**New files are always little-endian.**  The crate defaults to LE with
NVERSION=20141.  Reading handles both endiannesses transparently.

**Permissive mode** enables lenient header parsing for legacy / non-standard
files.  Non-critical issues become warnings instead of errors.

**Compression is transparent on read** — `open()` auto-detects gzip/bzip2 from
magic bytes and decompresses the whole file into memory.  Hard cap of
`DEFAULT_MAX_DECOMPRESSED_BYTES` (256 GiB) prevents bombs.

**`finalize()` rewrites the header** — the header is written optimistically at
file creation and rewritten at the end to capture any modifications (updated
stats, labels).  Every MRC file should call `finalize()`.
