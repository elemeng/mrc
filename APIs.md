# mrc API Reference

A zero-copy, zero-allocation MRC-2014 file format reader/writer for Rust.

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `std` | Standard library support, file I/O | ✅ |
| `mmap` | Memory-mapped I/O via memmap2 | ✅ |
| `file` | File operations | ✅ |
| `f16` | Half-precision float (requires nightly) | ✅ |

---

## Core Types

### Mode

Data type enumeration for MRC files.

```rust
pub enum Mode {
    Int8 = 0,           // Signed 8-bit integer
    Int16 = 1,          // Signed 16-bit integer
    Float32 = 2,        // 32-bit float (most common)
    Int16Complex = 3,   // Complex 16-bit integer
    Float32Complex = 4, // Complex 32-bit float
    Uint16 = 6,         // Unsigned 16-bit integer
    Float16 = 12,       // 16-bit float (requires f16 feature)
    Packed4Bit = 101,   // 4-bit packed data
}
```

| Method | Signature |
|--------|-----------|
| `from_i32` | `fn from_i32(mode: i32) -> Option<Self>` |
| `byte_size` | `const fn byte_size(self) -> usize` |
| `is_complex` | `const fn is_complex(self) -> bool` |
| `is_integer` | `const fn is_integer(self) -> bool` |
| `is_float` | `const fn is_float(self) -> bool` |
| `is_supported` | `fn is_supported(self) -> bool` |

---

### FileEndian

File endianness detection and conversion.

```rust
pub enum FileEndian {
    Little,
    Big,
}
```

| Method | Signature |
|--------|-----------|
| `native` | `const fn native() -> Self` |
| `is_native` | `const fn is_native(self) -> bool` |
| `from_machst` | `fn from_machst(machst: &[u8; 4]) -> Option<Self>` |
| `from_machst_or_little` | `fn from_machst_or_little(machst: &[u8; 4]) -> (Self, bool)` |
| `to_machst` | `const fn to_machst(self) -> [u8; 4]` |
| `convert_i32_to_native` | `fn convert_i32_to_native(self, value: i32) -> i32` |
| `convert_f32_to_native` | `fn convert_f32_to_native(self, value: f32) -> f32` |

---

### AxisMap

Axis ordering for 3D volumes.

```rust
pub struct AxisMap {
    pub column: usize,   // Fastest varying dimension
    pub row: usize,      // Middle dimension
    pub section: usize,  // Slowest varying dimension
}
```

| Method | Signature |
|--------|-----------|
| `new` | `const fn new(mapc: i32, mapr: i32, maps: i32) -> Self` |
| `try_new` | `fn try_new(mapc: i32, mapr: i32, maps: i32) -> Result<Self, Error>` |
| `is_standard` | `fn is_standard(&self) -> bool` |
| `validate` | `fn validate(&self) -> bool` |
| `axis_index` | `fn axis_index(&self, dim: usize) -> Option<usize>` |
| `strides` | `fn strides(&self, shape: [usize; 3]) -> [usize; 3]` |

**Standard mapping**: column=1, row=2, section=3 (X, Y, Z order)

---

### Error

Error type for all operations.

```rust
pub enum Error {
    InvalidHeader,
    InvalidMode,
    InvalidDimensions,
    InvalidAxisMap,
    TypeMismatch,
    WrongEndianness,
    MisalignedData { required: usize, actual: usize },
    BufferTooSmall { expected: usize, got: usize },
    IndexOutOfBounds { index: usize, length: usize },
    Io(String),                    // [std]
    Mmap,                          // [mmap]
    FeatureDisabled { feature: &'static str },
    UnknownEndianness,
}
```

---

## Header Types

### RawHeader

1024-byte MRC header with direct memory mapping.

```rust
#[repr(C, align(4))]
pub struct RawHeader {
    pub nx: i32,           // Columns
    pub ny: i32,           // Rows
    pub nz: i32,           // Sections
    pub mode: i32,         // Data type
    pub nxstart: i32,
    pub nystart: i32,
    pub nzstart: i32,
    pub mx: i32,           // Grid samples X
    pub my: i32,           // Grid samples Y
    pub mz: i32,           // Grid samples Z
    pub xlen: f32,         // Cell dimension X (Å)
    pub ylen: f32,         // Cell dimension Y (Å)
    pub zlen: f32,         // Cell dimension Z (Å)
    pub alpha: f32,        // Cell angle α (degrees)
    pub beta: f32,         // Cell angle β (degrees)
    pub gamma: f32,        // Cell angle γ (degrees)
    pub mapc: i32,         // Column axis mapping
    pub mapr: i32,         // Row axis mapping
    pub maps: i32,         // Section axis mapping
    pub dmin: f32,         // Minimum density
    pub dmax: f32,         // Maximum density
    pub dmean: f32,        // Mean density
    pub ispg: i32,         // Space group
    pub nsymbt: i32,       // Extended header size
    pub extra: [u8; 100],  // EXTTYP at 0-3, NVERSION at 4-7
    pub origin: [f32; 3],  // Origin coordinates
    pub map: [u8; 4],      // "MAP " identifier
    pub machst: [u8; 4],   // Machine stamp (endianness)
    pub rms: f32,          // RMS deviation
    pub nlabl: i32,        // Number of labels
    pub label: [u8; 800],  // 10 × 80 character labels
}
```

| Method | Signature |
|--------|-----------|
| `new` | `fn new() -> Self` |
| `zeroed` | `fn zeroed() -> Self` |
| `mode` | `fn mode(&self) -> Option<Mode>` |
| `data_size` | `fn data_size(&self) -> usize` |
| `data_offset` | `fn data_offset(&self) -> usize` |
| `has_valid_map` | `fn has_valid_map(&self) -> bool` |
| `validate` | `fn validate(&self) -> bool` |
| `exttyp` | `fn exttyp(&self) -> [u8; 4]` |
| `nversion` | `fn nversion(&self, endian: FileEndian) -> i32` |

---

### Header

Validated header with native-endian values.

```rust
pub struct Header {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub mode: Mode,
    pub nxstart: i32,
    pub nystart: i32,
    pub nzstart: i32,
    pub mx: i32,
    pub my: i32,
    pub mz: i32,
    pub xlen: f32,
    pub ylen: f32,
    pub zlen: f32,
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
    pub axis_map: AxisMap,
    pub dmin: f32,
    pub dmax: f32,
    pub dmean: f32,
    pub ispg: i32,
    pub nsymbt: usize,
    pub exttyp: [u8; 4],
    pub nversion: i32,
    pub file_endian: FileEndian,
    pub file_endian_detected: bool,
    pub xorigin: f32,
    pub yorigin: f32,
    pub zorigin: f32,
    pub rms: f32,
    pub nlabl: i32,
    pub label: [u8; 800],
}
```

| Method | Signature |
|--------|-----------|
| `new` | `fn new() -> Self` |
| `voxel_count` | `fn voxel_count(&self) -> usize` |
| `dimensions` | `fn dimensions(&self) -> (usize, usize, usize)` |
| `voxel_size` | `fn voxel_size(&self) -> (f32, f32, f32)` |
| `data_size` | `fn data_size(&self) -> usize` |
| `file_size` | `fn file_size(&self) -> usize` |
| `data_offset` | `fn data_offset(&self) -> usize` |
| `set_exttyp` | `fn set_exttyp(&mut self, value: [u8; 4])` |
| `set_exttyp_str` | `fn set_exttyp_str(&mut self, value: &str) -> Result<(), &'static str>` |
| `set_nversion` | `fn set_nversion(&mut self, value: i32)` |
| `set_axis_map` | `fn set_axis_map(&mut self, mapc: i32, mapr: i32, maps: i32) -> Result<(), Error>` |
| `set_dimensions` | `fn set_dimensions(&mut self, nx: usize, ny: usize, nz: usize)` |
| `set_cell_dimensions` | `fn set_cell_dimensions(&mut self, xlen: f32, ylen: f32, zlen: f32)` |
| `set_origin` | `fn set_origin(&mut self, x: f32, y: f32, z: f32)` |
| `set_statistics` | `fn set_statistics(&mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32)` |
| `label_str` | `fn label_str(&self, index: usize) -> Option<&str>` |

**Conversions**:
- `TryFrom<RawHeader>` - Validates and converts raw header
- `From<Header> for RawHeader` - Encodes to raw bytes

---

## Voxel Types

### Voxel Trait

Base trait for all voxel types.

```rust
pub trait Voxel: Copy + Send + Sync + 'static {
    const MIN: Self;
    const MAX: Self;
}
```

Implemented for: `i8`, `i16`, `u16`, `f32`, `half::f16` [f16], `ComplexI16`, `ComplexF32`

### RealVoxel Trait

Trait for real-valued voxels with float conversion.

```rust
pub trait RealVoxel: ScalarVoxel {
    fn from_f32(v: f32) -> Self;
    fn to_f32(self) -> f32;
}
```

### Complex Types

```rust
pub struct ComplexI16 {
    pub re: i16,
    pub im: i16,
}

pub struct ComplexF32 {
    pub re: f32,
    pub im: f32,
}

// Type aliases
pub type Int16Complex = ComplexI16;
pub type Float32Complex = ComplexF32;
```

---

## Encoding Trait

Endianness-aware encoding/decoding.

```rust
pub trait Encoding: Voxel {
    const MODE: Mode;
    const SIZE: usize;
    
    // Checked methods
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self;
    fn encode(self, endian: FileEndian, bytes: &mut [u8]);
    fn decode_checked(endian: FileEndian, bytes: &[u8]) -> Result<Self, Error>;
    fn encode_checked(self, endian: FileEndian, bytes: &mut [u8]) -> Result<(), Error>;
    
    // Unchecked (unsafe) methods
    unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self;
    unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]);
}
```

---

## Storage Traits [std]

### Storage (Read-only)

```rust
pub trait Storage {
    type Item: Copy;
    
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { ... }
    fn as_slice(&self) -> &[Self::Item];
}
```

### StorageMut (Read-write)

```rust
pub trait StorageMut: Storage {
    fn as_slice_mut(&mut self) -> &mut [Self::Item];
}
```

### Implementations

```rust
// In-memory vector storage
pub struct VecStorage<T> { /* private */ }
impl<T: Copy> Storage for VecStorage<T> { ... }
impl<T: Copy> StorageMut for VecStorage<T> { ... }

// Memory-mapped read-only [mmap]
pub struct MmapStorage<T> { /* private */ }
impl<T: Copy + Pod> Storage for MmapStorage<T> { ... }

// Memory-mapped read-write [mmap]
pub struct MmapStorageMut<T> { /* private */ }
impl<T: Copy + Pod> Storage for MmapStorageMut<T> { ... }
impl<T: Copy + Pod> StorageMut for MmapStorageMut<T> { ... }
```

---

## Volume [std]

Generic N-dimensional volume with const generics.

```rust
pub struct Volume<T, S, const D: usize = 3> { /* private */ }

// Type aliases
pub type Image2D<T, S> = Volume<T, S, 2>;
pub type VecVolume<T, D = 3> = Volume<T, Vec<u8>, D>;
pub type MmapVolume<T, D = 3> = Volume<T, Mmap, D>;        // [mmap]
pub type MmapVolumeMut<T, D = 3> = Volume<T, MmapMut, D>;  // [mmap]
```

### 3D Volume Methods

| Method | Signature |
|--------|-----------|
| `new` | `fn new(header: Header, storage: S) -> Result<Self, Error>` |
| `from_data` | `fn from_data(nx: usize, ny: usize, nz: usize, endian: FileEndian, storage: S) -> Result<Self, Error>` |
| `header` | `fn header(&self) -> &Header` |
| `shape` | `fn shape(&self) -> &[usize; D]` |
| `dimensions` | `fn dimensions(&self) -> (usize, usize, usize)` |
| `len` | `fn len(&self) -> usize` |
| `get` | `fn get(&self, index: usize) -> T` |
| `get_checked` | `fn get_checked(&self, index: usize) -> Option<T>` |
| `get_at` | `fn get_at(&self, x: usize, y: usize, z: usize) -> T` |
| `get_at_checked` | `fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<T>` |
| `iter` | `fn iter(&self) -> impl Iterator<Item = T> + '_` |
| `as_bytes` | `fn as_bytes(&self) -> &[u8]` |
| `as_bytes_mut` | `fn as_bytes_mut(&mut self) -> &mut [u8]` (requires S: AsMut) |
| `set` | `fn set(&mut self, index: usize, value: T)` (requires S: AsMut) |
| `set_at` | `fn set_at(&mut self, x: usize, y: usize, z: usize, value: T)` (requires S: AsMut) |

### 2D Image Methods

| Method | Signature |
|--------|-----------|
| `new_2d` | `fn new_2d(nx: usize, ny: usize, endian: FileEndian, storage: S) -> Result<Self, Error>` |
| `get_pixel` | `fn get_pixel(&self, x: usize, y: usize) -> T` |
| `get_pixel_checked` | `fn get_pixel_checked(&self, x: usize, y: usize) -> Option<T>` |

---

## VolumeData [std]

Dynamic dispatch for runtime mode handling.

```rust
pub enum VolumeData {
    I8(Volume<i8, Vec<u8>>),
    I16(Volume<i16, Vec<u8>>),
    F32(Volume<f32, Vec<u8>>),
    ComplexI16(Volume<Int16Complex, Vec<u8>>),
    ComplexF32(Volume<Float32Complex, Vec<u8>>),
    U16(Volume<u16, Vec<u8>>),
    F16(Volume<half::f16, Vec<u8>>),  // [f16]
}
```

| Method | Signature |
|--------|-----------|
| `from_bytes` | `fn from_bytes(header: Header, data: Vec<u8>) -> Result<Self, Error>` |
| `mode` | `fn mode(&self) -> Mode` |
| `header` | `fn header(&self) -> &Header` |
| `dimensions` | `fn dimensions(&self) -> (usize, usize, usize)` |
| `len` | `fn len(&self) -> usize` |
| `as_f32` | `fn as_f32(&self) -> Option<&Volume<f32, Vec<u8>>>` |
| `as_i16` | `fn as_i16(&self) -> Option<&Volume<i16, Vec<u8>>>` |
| `as_u16` | `fn as_u16(&self) -> Option<&Volume<u16, Vec<u8>>>` |
| `to_f32_vec` | `fn to_f32_vec(&self) -> Option<Vec<f32>>` |

---

## Extended Header [std]

```rust
pub enum ExtType {
    Ccp4,    // "CCP4"
    Mrco,    // "MRCO"
    Seri,    // "SERI"
    Agar,    // "AGAR"
    Fei1,    // "FEI1"
    Fei2,    // "FEI2"
    Hdf5,    // "HDF5"
    Unknown,
}

pub struct ExtendedHeader {
    pub ext_type: ExtType,
    pub data: Vec<u8>,
}
```

---

## File I/O [std]

### MrcReader

Read MRC files.

```rust
pub struct MrcReader { /* private */ }
```

| Method | Signature |
|--------|-----------|
| `open` | `fn open(path: impl AsRef<Path>) -> Result<Self, Error>` |
| `header` | `fn header(&self) -> &Header` |
| `ext_header` | `fn ext_header(&self) -> &[u8]` |
| `ext_header_parsed` | `fn ext_header_parsed(&self) -> ExtendedHeader` |
| `mode` | `fn mode(&self) -> Mode` |
| `dimensions` | `fn dimensions(&self) -> (usize, usize, usize)` |
| `read_data` | `fn read_data(&mut self) -> Result<Vec<u8>, Error>` |
| `read_volume` | `fn read_volume<T: Voxel + Encoding>(&mut self) -> Result<Volume<T, Vec<u8>>, Error>` |
| `read` | `fn read(&mut self) -> Result<VolumeData, Error>` |

**Example**:
```rust
use mrc::{MrcReader, Mode};

// Typed reading (compile-time check)
let mut reader = MrcReader::open("data.mrc")?;
if reader.mode() == Mode::Float32 {
    let volume = reader.read_volume::<f32>()?;
    let value = volume.get_at(10, 20, 5);
}

// Dynamic reading (runtime dispatch)
let mut reader = MrcReader::open("data.mrc")?;
let data = reader.read()?;
match data {
    VolumeData::F32(vol) => { /* handle */ },
    VolumeData::I16(vol) => { /* handle */ },
    _ => {}
}
```

### MrcWriter

Write MRC files.

```rust
pub struct MrcWriter { /* private */ }
```

| Method | Signature |
|--------|-----------|
| `create` | `fn create(path: impl AsRef<Path>, header: Header) -> Result<Self, Error>` |
| `create_with_ext_header` | `fn create_with_ext_header(path: impl AsRef<Path>, header: Header, ext_header: &[u8]) -> Result<Self, Error>` |
| `builder` | `fn builder() -> MrcWriterBuilder` |
| `header` | `fn header(&self) -> &Header` |
| `header_mut` | `fn header_mut(&mut self) -> &mut Header` |
| `write_data` | `fn write_data(&mut self, data: &[u8]) -> Result<(), Error>` |
| `flush_header` | `fn flush_header(&mut self) -> Result<(), Error>` |

### MrcWriterBuilder

Builder pattern for creating MRC files.

```rust
pub struct MrcWriterBuilder { /* private */ }
```

| Method | Signature |
|--------|-----------|
| `new` | `fn new() -> Self` |
| `shape` | `fn shape(self, nx: usize, ny: usize, nz: usize) -> Self` |
| `mode` | `fn mode(self, mode: Mode) -> Self` |
| `voxel_size` | `fn voxel_size(self, dx: f32, dy: f32, dz: f32) -> Self` |
| `origin` | `fn origin(self, x: f32, y: f32, z: f32) -> Self` |
| `cell_angles` | `fn cell_angles(self, alpha: f32, beta: f32, gamma: f32) -> Self` |
| `ext_header` | `fn ext_header(self, data: Vec<u8>) -> Self` |
| `data` | `fn data(self, data: Vec<u8>) -> Self` |
| `write` | `fn write(self, path: impl AsRef<Path>) -> Result<(), Error>` |

**Example**:
```rust
use mrc::{MrcWriter, Mode};

// Using builder
MrcWriter::builder()
    .shape(512, 512, 256)
    .mode(Mode::Float32)
    .voxel_size(1.0, 1.0, 1.0)
    .data(my_data)
    .write("output.mrc")?;

// Manual creation
let mut header = Header::new();
header.set_dimensions(512, 512, 256);
header.mode = Mode::Float32;
header.xlen = 512.0;
header.ylen = 512.0;
header.zlen = 256.0;

let mut writer = MrcWriter::create("output.mrc", header)?;
writer.write_data(&data)?;
```

---

## no_std Usage

Disable default features for `no_std` environments:

```toml
[dependencies]
mrc = { version = "0.2", default-features = false }
```

Available types without `std`:
- `Mode`, `FileEndian`, `AxisMap`, `Error`
- `RawHeader`, `Header`
- `Voxel`, `Encoding` traits
- `ComplexI16`, `ComplexF32`

---

## Constants

| Constant | Value | Location |
|----------|-------|----------|
| `RawHeader::SIZE` | 1024 | MRC header size in bytes |
| `AxisMap::STANDARD` | `{column: 1, row: 2, section: 3}` | Standard X-Y-Z ordering |

---

## Byte Layout Reference

MRC2014 file layout:

```
| Offset    | Size     | Description          |
|-----------|----------|----------------------|
| 0         | 1024     | Header               |
| 1024      | nsymbt   | Extended header      |
| 1024+nsymbt | data   | Voxel data           |
```

Header byte offsets:

```
| Offset | Bytes | Field       |
|--------|-------|-------------|
| 0      | 4     | nx          |
| 4      | 4     | ny          |
| 8      | 4     | nz          |
| 12     | 4     | mode        |
| 16-27  | 12    | start coords|
| 28-39  | 12    | grid samples|
| 40-51  | 12    | cell dims   |
| 52-63  | 12    | cell angles |
| 64-75  | 12    | axis map    |
| 76-87  | 12    | statistics  |
| 88     | 4     | ispg        |
| 92     | 4     | nsymbt      |
| 96-99  | 4     | EXTTYP      |
| 100-103| 4     | NVERSION    |
| 104-195| 92    | padding     |
| 196-207| 12    | ORIGIN      |
| 208-211| 4     | "MAP "      |
| 212-215| 4     | MACHST      |
| 216-219| 4     | RMS         |
| 220-223| 4     | NLABL       |
| 224-1023| 800  | LABELS      |
```
