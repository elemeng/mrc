# `mrc` API Reference

> MRC-2014 file format library for cryo-EM / cryo-ET.  
> This document describes the **public API surface** — what's available to you as a user of the crate.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Top-Level Functions](#top-level-functions)
3. [Readers](#readers)
   - [`Reader`](#reader) — auto-selects mmap or buffered
4. [Writers](#writers)
   - [`WriterBuilder` / `Writer`](#writerbuilder--writer) — standard file I/O
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
use mrc::{open, create, VoxelBlock};

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

The auto-selecting reader. For files, it prefers memory-mapped I/O (zero-copy)
and falls back to buffered I/O. Also accepts in-memory buffers and generic
`Read` streams via `from_reader`/`from_bytes`.

All iterator and conversion methods (`slices`, `slabs`, `tiles`,
`subregion`, `read_volume`, `convert`, `slices_u8`, etc.) are available
as **inherent methods** — no trait imports needed for normal use.

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
| `Reader::from_reader(r)` | `Result<Reader>` | Read from any `Read` source (memory, network, etc.) |
| `Reader::from_reader_permissive(r)` | `Result<(Reader, Vec<String>)>` | Permissive read from any `Read` source |
| `Reader::from_bytes(data)` | `Result<Reader>` | Parse from in-memory `Vec<u8>` |
| `Reader::from_bytes_permissive(data)` | `Result<(Reader, Vec<String>)>` | Permissive parse from `Vec<u8>` |
| `reader.shape()` | `VolumeShape` | Volume dimensions `(nx, ny, nz)` |
| `reader.mode()` | `Mode` | Voxel data mode |
| `reader.header()` | `&Header` | Reference to parsed header |
| `reader.endian()` | `FileEndian` | Detected byte order |
| `reader.data_bytes()` | `&[u8]` | Raw voxel data bytes |
| `reader.ext_header_bytes()` | `&[u8]` | Extended header bytes (empty if none) |
| `reader.read_block_bytes(offset, shape)` | `Result<Vec<u8>>` | Read raw bytes for any sub-block |
| `reader.validate_header_stats()` | `Result<()>` | Cross-check header stats vs actual data (1% tolerance) |
| `reader.parse_extended_header()` | `ExtHeaderData` | Auto-detect EXTTYP and parse extended header bytes |
| `reader.fei1_metadata()` | `Option<Vec<Fei1Metadata>>` | Parse FEI1 records from extended header |
| `reader.fei2_metadata()` | `Option<Vec<Fei2Metadata>>` | Parse FEI2 records from extended header |
| `reader.ccp4_records()` | `Option<Vec<Ccp4Record>>` | Parse CCP4 symmetry records |
| `reader.mrco_records()` | `Option<Vec<MrcoRecord>>` | Parse MRCO legacy records |
| `reader.seri_records()` | `Option<Vec<SeriRecord>>` | Parse SerialEM records |
| `reader.agar_records()` | `Option<Vec<AgarRecord>>` | Parse Agard records |
| `reader.imod_metadata()` | `Option<ImodMetadata>` | Parse IMOD metadata from header `extra` bytes |

**All methods (inherent — no trait import needed):**

| Method | Returns | Description |
|---|---|---|
| `reader.subregion::<T>(offset, shape)` | `Result<VoxelBlock<T>>` | Read and decode typed sub-block at any offset |
| `reader.read_volume::<T>()` | `Result<VoxelBlock<T>>` | Read the entire volume as a single block |
| `reader.read_volume_u8()` | `Result<VoxelBlock<u8>>` | Read Packed4Bit volume as `u8` (nibble unpack) |
| `reader.slices::<T>()` | `RegionIter<'_, T, SliceStepper>` | One Z-plane at a time |
| `reader.slabs::<T>(k)` | `RegionIter<'_, T, SlabStepper>` | `k` contiguous Z-planes |
| `reader.tiles::<T>(shape)` | `RegionIter<'_, T, TileStepper>` | Arbitrary 3D tiles |
| `reader.volumes::<T>()` | `Result<RegionIter<'_, T, SlabStepper>>` | One sub-volume per step (volume stacks only) |
| `reader.slices_u8()` | iterator yielding `VoxelBlock<u8>` | Mode 6 (Uint16) or Mode 101 (Packed4Bit); narrows/nibble-unpacks to `u8` |
| `reader.slabs_u8(k)` | iterator yielding `VoxelBlock<u8>` | Same as `slices_u8` but `k` planes at a time |
| `reader.slices_mode0(interp)` | iterator yielding `VoxelBlock<f32>` | Mode 0 (Int8) only; signed or unsigned |
| `reader.slabs_mode0(k, interp)` | iterator yielding `VoxelBlock<f32>` | Same as `slices_mode0` but `k` planes at a time |
| `reader.convert::<T>()` | [`ConvertReader`] | Returns a wrapper; all reads auto-convert to type `T` |

Then use the wrapper's inherent methods:

| Method | Returns | Description |
|---|---|---|
| `reader.convert::<T>().slices()` | iterator yielding `VoxelBlock<T>` | Auto-convert any mode to target type `T` |
| `reader.convert::<T>().slabs(k)` | iterator yielding `VoxelBlock<T>` | Same as `slices` but `k` planes at a time |
| `reader.convert::<T>().tiles(shape)` | iterator yielding `VoxelBlock<T>` | Same as `slices` but arbitrary 3D tiles |
| `reader.convert::<T>().subregion(offset, shape)` | `Result<VoxelBlock<T>>` | Single block at given offset/shape, auto-converted |
| `reader.convert::<T>().read_volume()` | `Result<VoxelBlock<T>>` | Full volume as one block, auto-converted |
| `reader.convert::<T>().with_complex_strategy(s)` | `Self` (builder) | Configure complex‑mode reduction (RealPart, Magnitude, etc.) |
| `reader.convert::<T>().with_m0_interpretation(i)` | `Self` (builder) | Configure Mode 0 as Signed or Unsigned |
| `reader.convert::<T>().to_ndarray()` | `Result<Array3<T>>` (feature `ndarray`) | Full volume as an `ndarray::Array3` |

### Performance note: memory-mapped access

[`Reader::open`] automatically uses memory-mapped I/O (zero-copy, demand-paged)
when available (requires the `mmap` feature). The [`slab_as`](crate::Reader::slab_as)
method provides zero-copy typed access into the mmap when the file endianness
matches the host and the voxel type matches the file mode.

| Method | Returns | Description |
|---|---|---|
| `reader.slab_as::<T>(z, k)` | `Result<&[T]>` | Zero-copy typed access into the mmap |
| `reader.is_truncated()` | `bool` | `true` if permissive-mode file is shorter than header claims |

---

## Writers

### `WriterBuilder` / `Writer`

The standard writer. Created via `create(path)` or `WriterBuilder::new(path)`.

**Builder methods:**

```rust
let writer = create("out.mrc")
    .shape([nx, ny, nz])          // volume dimensions (also sets mx,my,mz)
    .mode::<f32>()                // voxel type (i8, i16, u16, f32, etc.)
    .mode_raw(101)                // set raw mode (e.g. Packed4Bit, no Voxel impl)
    .cell_lengths(xlen, ylen, zlen) // unit cell in Å
    .ispg(1)                      // space group
    .exttyp(*b"CCP4")             // 4-byte extended header type
    .nsymbt(1024)                 // extended header size in bytes
    .set_volume_stack(30)        // configure as volume stack (ispg=401, mz=30)
    .origin([0.0, 0.0, 0.0])     // origin coordinates
    .ext_header_bytes(vec![])     // raw extended header bytes (sets nsymbt)
    .compression(Compression::Best) // compression level for gzip/bzip2
    .finish()?;                   // → Result<Writer>
```

Additional builder methods behind feature flags:
- `.finish_mmap()?` → `Writer` backed by mmap (feature `mmap`)
- `.finish_gzip()?` → `Writer` backed by in-memory buffer, gzip-compressed on finalize (feature `gzip`)
- `.finish_bzip2()?` → `Writer` backed by in-memory buffer, bzip2-compressed on finalize (feature `bzip2`)

**Writer methods:**

| Method | Description |
|---|---|
| `Writer::from_writer(writer, header, ext)` | Create from any `Read + Write + Seek` target (e.g. `Cursor<Vec<u8>>`) |
| `Writer::from_writer_mmap(path, header, ext)` | Create mmap writer from raw `Header` (feature `mmap`) |
| `Writer::from_writer_gzip(path, header, ext, comp)` | Create gzip writer from raw `Header` (feature `gzip`) |
| `Writer::from_writer_bzip2(path, header, ext, comp)` | Create bzip2 writer from raw `Header` (feature `bzip2`) |
| `writer.shape()` | Volume dimensions |
| `writer.mode()` | Voxel mode |
| `writer.header()` | Read-only reference to header |
| `writer.header_mut()` | Mutable reference to header (modify before `finalize`) |
| `writer.write_block::<T>(&block)` | Write a typed voxel block. `T` must match file mode |
| `writer.write_u8_block(&block)` | Convenience: write `VoxelBlock<u8>` to a Uint16 file (auto-widens) |
| `writer.write_u4_block(&block)` | Convenience: write `VoxelBlock<u8>` to a Packed4Bit file (auto-packs, values must be 0–15) |
| `writer.write_block_as(&block)` | Write with auto-conversion to file's mode: `f32` → `i8`/`i16`/`u16`/`f16` (clamped) |
| `writer.write_block_parallel::<T>(&block)` | Parallel-encoded write (feature `parallel`; contiguous XY slabs only) |
| `writer.finalize()` | **Required.** Rewrites the header with final metadata (updated stats, labels). The header is written optimistically at file creation and rewritten here — without it the header is stale and tools may display wrong contrast. |
| `writer.update_header_stats()` | Scan all data from disk, compute dmin/dmax/dmean/rms, update header — ⚠️ re-reads entire data block from disk |

### Memory-mapped Writer (`Writer` with mmap)

Memory-mapped writer. Created via `WriterBuilder::finish_mmap()`.
Requires `mmap` feature.

Same API as the file-backed `Writer` (`write_block`, `write_block_as`, `write_u8_block`,
`write_u4_block`, `write_block_parallel`, `finalize`, `update_header_stats`).

**`finalize()` is required** — flushes the mmap to disk. Without it the header
is stale (density statistics missing, tools display wrong contrast).

### Compressed Writers (`Writer` with gzip/bzip2)

Created via `WriterBuilder::finish_gzip()` / `finish_bzip2()`.

**Important:** Compressed writers buffer the **entire file** in memory and compress
only on `finalize`. **`finalize()` is required** — without it nothing is written
to disk. Not suitable for large volumes that exceed RAM.

Full API: `shape()`, `mode()`, `header()`, `header_mut()`, `write_block()`, `write_block_as()`, `write_u8_block()`, `write_u4_block()`,
`update_header_stats()` (reads from in-memory buffer),
`finalize()` (writes compressed data to disk).

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
| `header.decode_from_bytes_with_info(bytes)` | `(Header, Option<EndianFallbackWarning>)` | Parse with endian fallback diagnostics |
| `header.encode_to_bytes(&mut [u8; 1024])` | `()` | Encode to raw bytes |
| `header.exttyp()` | `[u8; 4]` | Extended header type from `extra[8..12]` |
| `header.set_exttyp(value)` | `()` | Set extended header type |
| `header.exttyp_str()` | `Result<&str>` | Extended header type as string (`exttyp` decoded as UTF-8) |
| `header.set_exttyp_str(value)` | `Result<()>` | Set extended header type from a string |
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
| `header.sampling()` | `[i32; 3]` | `[mx, my, mz]` |
| `header.density_stats()` | `(f32, f32, f32, f32)` | `(dmin, dmax, dmean, rms)` |
| `header.is_standard_map()` | `bool` | `true` when MAP field is exactly `"MAP "` |
| `header.label_at(i)` | `Option<&str>` | Trimmed label at index `i`, or `None` if empty |
| `header.cell_volume()` | `f64` | Unit cell volume in Å³ (triclinic formula) |
| `header.nstart()` | `[i32; 3]` | `[nxstart, nystart, nzstart]` |
| `header.cell_lengths()` | `[f32; 3]` | `[xlen, ylen, zlen]` |
| `header.cell_angles()` | `[f32; 3]` | `[alpha, beta, gamma]` |
| `header.logical_shape()` | `[usize; 4]` | Python-style `(nvolumes, mz, ny, nx)` |
| `header.detect_imod()` | `Option<ImodInfo>` | Detect IMOD stamp in `extra` bytes |
| `header.is_y_inverted()` | `bool` | `true` when `mapr == -2` (IMOD convention) |

**`HeaderBuilder` methods:**

```rust
HeaderBuilder::new()
    .shape([nx, ny, nz])         // set dimensions + mx,my,mz
    .mode::<f32>()               // set voxel type → mode
    .mode_raw(101)               // set raw mode (e.g. Packed4Bit, no Voxel impl)
    .cell_lengths(x, y, z)       // cell dimensions in Å
    .cell_angles(a, b, g)        // cell angles in degrees
    .ispg(n)                     // space group
    .exttyp(*b"CCP4")            // extended header type
    .nsymbt(n)                   // extended header size
    .origin([x, y, z])           // origin
    .nstart([x, y, z])           // sub-volume origin in pixels (nxstart..)
    .sampling([mx, my, mz])      // cell sampling rates (independent of shape)
    .axis_mapping([1, 2, 3])     // column/row/section axis mapping (mapc/r/s)
    .add_label("my volume")      // append a text label
    .set_volume_stack(30)        // configure as volume stack (ispg=401, mz=30)
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
| `VolumeShape::from_header(&Header)` | `Result<VolumeShape>` | From parsed header (negative dims → `Err`) |
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
| `VoxelBlock::try_new(offset, shape, data)` | `Result<Self>` | Like `new`, validates data length matches shape |
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
  [`convert::<f32>()`](Reader::convert) / [`convert::<T>()`](Reader::convert)
  convert directly to `f32` or any target type via `.slices()` / `.read_volume()` /
  `.subregion()` — correctly handles multi-slice volumes and sub-block shapes.
- Writing: [`write_u4_block`](Writer::write_u4_block) packs `u8` values
  two-per-byte.

```rust
pub enum ComplexToRealStrategy { RealPart, ImaginaryPart, Magnitude, Phase }
pub enum M0Interpretation { Signed, Unsigned }
```

`M0Interpretation` controls how Mode 0 (8-bit) data is read: `Signed` treats raw bytes as `i8`, `Unsigned` as `u8`.

### Extended Header Types

```rust
pub enum ExtHeaderType { Ccp4, Mrco, Seri, Agar, Fei1, Fei2, Hdf5, Unknown([u8; 4]) }

impl ExtHeaderType {
    pub fn from_exttyp(exttyp: [u8; 4]) -> Self;
    pub fn from_header(header: &Header) -> Self;
}
```

Maps the 4-byte EXTTYP identifier from `extra[8..12]` to a Rust enum for generic dispatch.
`Unknown` captures any unrecognized identifier.

```rust
pub enum ExtHeaderData {
    Ccp4(Vec<Ccp4Record>),
    Mrco(Vec<MrcoRecord>),
    Seri(Vec<SeriRecord>),
    Agar(Vec<AgarRecord>),
    Fei1(Vec<Fei1Metadata>),
    Fei2(Vec<Fei2Metadata>),
    None,
}

impl ExtHeaderData {
    pub fn parse(ext_type: ExtHeaderType, bytes: &[u8]) -> Self;
    pub fn from_header(header: &Header, bytes: &[u8]) -> Self;
}
```

Returned by `reader.parse_extended_header()`. Auto-routes to the correct parser based
on the `exttyp` detected from the header.

### IMOD Metadata

```rust
pub struct ImodInfo { pub bytes_are_signed: bool }

pub enum ImodImageType { Mono, Tilt, Tilts, Lina, Lins }

pub struct ImodMetadata {
    pub bytes_are_signed: bool,
    pub imod_flags: u16,
    pub image_type: ImodImageType,
    pub tilt_axis: u8,
    pub tilt_increment: f32,
    pub start_angle: f32,
    pub original_angles: [f32; 3],
    pub current_angles: [f32; 3],
    pub x_origin: f32,
    pub y_origin: f32,
    pub z_origin: f32,
    pub x_cell_size: f32,
    pub y_cell_size: f32,
    pub z_cell_size: f32,
}

pub fn parse_imod_metadata(header: &Header) -> Option<ImodMetadata>;
```

Parsed from the MRC header `extra` bytes when the IMOD stamp is present.
Accessible via `reader.imod_metadata()` on any reader.

---

## Iterators

### `RegionIter` and Steppers

`RegionIter<'a, T, S>` is a **lazy iterator** that yields `Result<VoxelBlock<T>>`.
Created by reader methods like `.slices::<f32>()`.

```rust
pub trait Stepper { /* internal, #[doc(hidden)] */ }

pub struct SliceStepper;    // one Z-plane at a time
pub struct SlabStepper;     // k contiguous Z-planes
pub struct TileStepper;     // arbitrary 3D tiles
```

| Stepper | Construct | Behaviour |
|---|---|---|
| `SliceStepper` | `SliceStepper::default()` | Steps `z` from 0..nz, block shape = `[nx, ny, 1]` |
| `SlabStepper` | `SlabStepper::new(k)` | Steps `k` slices at a time, block shape = `[nx, ny, k]` |
| `TileStepper` | `TileStepper::new([sx, sy, sz])` | Raster-scans volume in `[sx, sy, sz]` tiles |

You don't normally interact with `RegionIter` directly — just use the reader
methods returned by `slices()`, `slabs()`, `tiles()` etc.:

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

### Other Extended Header Formats

```rust
use mrc::{
    Ccp4Record, MrcoRecord, SeriRecord, AgarRecord,
    CCP4_RECORD_SIZE, MRCO_RECORD_SIZE, SERI_RECORD_SIZE, AGAR_RECORD_SIZE,
    parse_ccp4_records, parse_mrco_records, parse_seri_records, parse_agar_records,
};
```

```rust
pub const CCP4_RECORD_SIZE: usize = 80;
pub const MRCO_RECORD_SIZE: usize = 80;
pub const SERI_RECORD_SIZE: usize = 256;
pub const AGAR_RECORD_SIZE: usize = 1024;

// Parse extended header bytes into typed records
pub fn parse_ccp4_records(bytes: &[u8]) -> Option<Vec<Ccp4Record>>;
pub fn parse_mrco_records(bytes: &[u8]) -> Option<Vec<MrcoRecord>>;
pub fn parse_seri_records(bytes: &[u8]) -> Option<Vec<SeriRecord>>;
pub fn parse_agar_records(bytes: &[u8]) -> Option<Vec<AgarRecord>>;
```

| Format | Record size | Typical use |
|--------|-------------|-------------|
| CCP4 | 80 bytes | CCP4 suite symmetry records |
| MRCO | 80 bytes | Legacy MRC format records |
| SERI | 256 bytes | SerialEM tilt-series metadata |
| AGAR | 1024 bytes | Agard-style microscope metadata |

Access via `reader.ccp4_records()`, `reader.mrco_records()`, etc. on any open
reader, or directly via `parse_*_records()` for raw byte slices.

`Ccp4Record` has an `as_str()` method returning trimmed symmetry text.
`SeriRecord` exposes the `alpha_tilt` field directly; all other record types
store raw bytes for caller interpretation.

---

## Error Types

### `Error` — top-level error enum

| Variant | When it occurs |
|---|---|
| `Io(std::io::Error)` | Underlying I/O failure |
| `InvalidHeader` | Malformed header |
| `UnsupportedMode` | Mode not recognized |
| `BoundsError { offset?, shape?, volume? }` | Block outside volume bounds (optional context) |
| `TypeMismatch { expected, actual }` | Byte size mismatch |
| `ValueOutOfRange { value, max }` | Voxel value exceeds target type range |
| `BlockShapeMismatch { expected, actual }` | Data length ≠ block volume |
| `ModeMismatch { file_mode, requested_mode, offset? }` | Requested type ≠ file mode (optional offset) |
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
conversion infrastructure is used internally by `reader.convert::<T>()` which
automatically converts any MRC mode to the target type via `.slices()`, `.slabs()`,
`.tiles()`, `.subregion()`, and `.read_volume()`.

---

## Feature Flags

| Feature | Default | What it enables |
|---|---|---|
| `mmap` | ✅ | Memory-mapped I/O (auto-selected by `Reader::open`, `WriterBuilder::finish_mmap()`) |
| `f16` | ✅ | `half::f16` type, `Mode::Float16`, `write_block_as()` for f32→f16 |
| `simd` | ✅ | AVX2/NEON accelerated integer↔f32, f16↔f32, byte-swap, f32 statistics, and f32→integer clamping |
| `parallel` | ✅ | `write_block_parallel()` using `rayon` |
| `gzip` | ✅ | Gzip auto-detection, `Reader::open_gzip()`, compressed writer |
| `bzip2` | ❌ | Bzip2 auto-detection, `Reader::open_bzip2()`, compressed writer |
| `ndarray` | ❌ | Return volumes as `ndarray::Array3<T>` via `to_ndarray()` |
| `serde` | ❌ | Serialize/Deserialize for `Header`, `Mode`, `VolumeShape`, `ValidationReport`, and other public types |

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

> **Large compressed files:** If the uncompressed data exceeds available RAM,
> decompress with `gunzip` or `bunzip2` first, then use `Reader::open` for
> zero-copy access — the OS pages data on demand.

[`DEFAULT_MAX_DECOMPRESSED_BYTES`]: #top-level-functions

**`finalize()` rewrites the header** — the header is written optimistically at
file creation and rewritten at the end to capture any modifications (e.g. updated
stats, labels). Every MRC file should call `finalize()`.
