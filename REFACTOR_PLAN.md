# MRC Crate Refactor Plan

## Executive Summary

Refactor from runtime-mode-dispatch to compile-time generic architecture.

**Goal**: Zero redundancy, zero-copy, compile-time safety, SIMD-friendly

---

## Current vs Proposed Architecture

| Layer | Current | Proposed | Benefit |
|-------|---------|----------|---------|
| Header | Single struct | RawHeader + Header | Validation separated |
| Mode | Enum + runtime check | Enum + Encoding trait | Dispatch at compile time |
| Voxel | VoxelType trait | Voxel + ScalarVoxel + ComplexVoxel | Trait hierarchy |
| Storage | DataBlock (bytes + mode) | Storage<T> trait | Backend abstraction |
| Container | MrcView (runtime mode) | Volume<T, S, D> | Compile-time type safety |
| IO | MrcFile methods | MrcReader + MrcWriter | Builder pattern |

---

## Module Structure

```
src/
├── lib.rs                    # Crate docs + re-exports
│
├── header/
│   ├── mod.rs               # Re-exports
│   ├── raw.rs               # RawHeader #[repr(C)]
│   └── validated.rs         # Header + AxisMap
│
├── mode.rs                  # Mode enum
│
├── encoding.rs              # Encoding trait + impls
│
├── voxel/
│   ├── mod.rs               # Re-exports
│   ├── traits.rs            # Voxel, ScalarVoxel, ComplexVoxel
│   └── types.rs             # Complex<T>, impls
│
├── storage/
│   ├── mod.rs               # Re-exports + Storage trait
│   ├── vec.rs               # VecStorage<T>
│   ├── mmap.rs              # MmapStorage<T> (optional)
│   └── chunk.rs             # ChunkStorage<T> (streaming)
│
├── volume/
│   ├── mod.rs               # Re-exports
│   ├── core.rs              # Volume<T, S, D>
│   ├── slicing.rs           # Slice views
│   └── iter.rs              # Iterators
│
├── io/
│   ├── mod.rs               # Re-exports
│   ├── reader.rs            # MrcReader
│   └── writer.rs            # MrcWriter + builder
│
├── axis.rs                  # AxisMap
├── extended.rs              # ExtendedHeader + ExtType
├── dynamic.rs               # VolumeData enum
└── error.rs                 # MrcError

test/
├── header_tests.rs
├── voxel_tests.rs
├── volume_tests.rs
├── io_tests.rs
└── integration_tests.rs
```

---

## Phase 1: Core Traits

### 1.1 Voxel Traits

```rust
// src/voxel/traits.rs

/// Base trait for all voxel types
pub trait Voxel: Copy + Send + Sync + 'static {
    /// Minimum value for this voxel type
    const MIN: Self;
    /// Maximum value for this voxel type
    const MAX: Self;
}

/// Marker for scalar (non-complex) voxels
pub trait ScalarVoxel: Voxel {}

/// Marker for complex voxels
pub trait ComplexVoxel: Voxel {
    type Real: ScalarVoxel;
}

/// Marker for real (floating-point) voxels
pub trait RealVoxel: ScalarVoxel {
    fn from_f32(f: f32) -> Self;
    fn to_f32(self) -> f32;
}
```

### 1.2 Encoding Trait

```rust
// src/encoding.rs

use crate::{FileEndian, Mode, Voxel};

/// Encoding converts between file bytes and voxel values
pub trait Encoding: Voxel {
    /// The MRC mode this encoding handles
    const MODE: Mode;
    
    /// Size in bytes for one voxel
    const SIZE: usize;
    
    /// Decode one voxel from file-endian bytes
    fn decode(file_endian: FileEndian, bytes: &[u8]) -> Self;
    
    /// Encode one voxel to file-endian bytes
    fn encode(self, file_endian: FileEndian, bytes: &mut [u8]);
    
    /// Decode slice for native endianness (zero-copy when possible)
    fn decode_slice_native(bytes: &[u8]) -> Option<&[Self]>;
}
```

### 1.3 Storage Trait

```rust
// src/storage/mod.rs

use crate::Voxel;

/// Storage backend abstraction
pub trait Storage<T: Voxel>: core::ops::Deref<Target = [T]> {
    /// Create storage from a Vec
    fn from_vec(data: Vec<T>, shape: [usize; 3]) -> Self;
    
    /// Get the shape
    fn shape(&self) -> [usize; 3];
    
    /// Get mutable access (if possible)
    fn as_mut_slice(&mut self) -> Option<&mut [T]>;
}

/// Owned storage using Vec
pub struct VecStorage<T: Voxel> {
    data: Vec<T>,
    shape: [usize; 3],
}

/// Memory-mapped storage (requires mmap feature)
#[cfg(feature = "mmap")]
pub struct MmapStorage<T: Voxel> {
    mmap: memmap2::Mmap,
    shape: [usize; 3],
    _marker: core::marker::PhantomData<T>,
}
```

---

## Phase 2: Header Split

### 2.1 RawHeader

```rust
// src/header/raw.rs

/// Raw 1024-byte MRC header with exact binary layout
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RawHeader {
    pub nx: i32,           // 0
    pub ny: i32,           // 4
    pub nz: i32,           // 8
    pub mode: i32,         // 12
    // ... all 1024 bytes
    pub machst: [u8; 4],   // 212-215
    pub map: [u8; 4],      // 224-227
    // ...
}

impl RawHeader {
    pub const SIZE: usize = 1024;
    
    /// Read from bytes (zero-copy)
    pub fn from_bytes(bytes: &[u8; 1024]) -> &Self {
        bytemuck::from_bytes(bytes)
    }
}
```

### 2.2 Validated Header

```rust
// src/header/validated.rs

use super::RawHeader;

/// Validated header with semantic access
#[derive(Debug, Clone)]
pub struct Header {
    // All fields as native types
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub mode: Mode,
    pub xlen: f32,
    pub ylen: f32,
    pub zlen: f32,
    // ...
    
    // Derived
    pub axis_map: AxisMap,
    pub file_endian: FileEndian,
}

impl TryFrom<RawHeader> for Header {
    type Error = MrcError;
    
    fn try_from(raw: RawHeader) -> Result<Self, Self::Error> {
        // Validate MAP string
        // Validate mode
        // Detect endianness
        // Parse axis mapping
        // ...
    }
}
```

### 2.3 AxisMap

```rust
// src/axis.rs

/// Axis permutation from file to logical
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisMap {
    /// Column axis (fastest varying)
    pub column: usize,  // MAPC
    /// Row axis
    pub row: usize,     // MAPR
    /// Section axis (slowest varying)
    pub section: usize, // MAPS
}

impl Default for AxisMap {
    fn default() -> Self {
        Self { column: 1, row: 2, section: 3 }
    }
}
```

---

## Phase 3: Volume Container

```rust
// src/volume/core.rs

use crate::{Header, Storage, Voxel, Encoding};

/// Typed volume with compile-time voxel type
pub struct Volume<T: Voxel, S: Storage<T>, const D: usize = 3> {
    storage: S,
    shape: [usize; D],
    strides: [usize; D],
    header: Header,
}

impl<T: Voxel + Encoding, S: Storage<T>, const D: usize> Volume<T, S, D> {
    /// Create from storage and header
    pub fn new(storage: S, header: Header) -> Result<Self, MrcError> {
        // Validate dimensions match
        // Calculate strides
    }
    
    /// Get voxel at coordinates
    #[inline]
    pub fn get(&self, coords: [usize; D]) -> T {
        let idx = self.coords_to_index(coords);
        self.storage[idx]
    }
    
    /// Get shape
    pub fn shape(&self) -> &[usize; D] { &self.shape }
    
    /// Get strides
    pub fn strides(&self) -> &[usize; D] { &self.strides }
    
    /// Convert coordinates to linear index
    #[inline]
    fn coords_to_index(&self, coords: [usize; D]) -> usize {
        let mut idx = 0;
        for i in 0..D {
            idx += coords[i] * self.strides[i];
        }
        idx
    }
}

// Common type aliases
pub type VecVolume<T, const D: usize = 3> = Volume<T, VecStorage<T>, D>;
pub type MmapVolume<T, const D: usize = 3> = Volume<T, MmapStorage<T>, D>;
```

---

## Phase 4: IO Layer

### 4.1 Reader

```rust
// src/io/reader.rs

pub struct MrcReader {
    header: Header,
    ext_header: Vec<u8>,
    data_offset: usize,
    file_endian: FileEndian,
}

impl MrcReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MrcError> { ... }
    
    /// Read volume with compile-time type
    pub fn read_volume<T: Voxel + Encoding>(&self) -> Result<VecVolume<T>, MrcError> {
        // Verify mode matches
        if self.header.mode != T::MODE {
            return Err(MrcError::ModeMismatch);
        }
        
        // Read bytes
        // Decode to Vec<T>
        // Create Volume
    }
    
    /// Read with memory mapping
    #[cfg(feature = "mmap")]
    pub fn mmap_volume<T: Voxel + Encoding>(&self) -> Result<MmapVolume<T>, MrcError> { ... }
    
    /// Read dynamic (runtime dispatch)
    pub fn read(&self) -> Result<VolumeData, MrcError> {
        match self.header.mode {
            Mode::Int8 => Ok(VolumeData::I8(self.read_volume::<i8>()?)),
            Mode::Int16 => Ok(VolumeData::I16(self.read_volume::<i16>()?)),
            Mode::Float32 => Ok(VolumeData::F32(self.read_volume::<f32>()?)),
            // ...
        }
    }
}
```

### 4.2 Writer

```rust
// src/io/writer.rs

pub struct MrcWriter {
    header: Header,
    data: VolumeData,
}

pub struct MrcWriterBuilder {
    shape: [usize; 3],
    mode: Mode,
    voxel_size: [f32; 3],
    origin: [f32; 3],
    data: Option<VolumeData>,
}

impl MrcWriterBuilder {
    pub fn shape(mut self, nx: usize, ny: usize, nz: usize) -> Self { ... }
    pub fn mode(mut self, mode: Mode) -> Self { ... }
    pub fn voxel_size(mut self, x: f32, y: f32, z: f32) -> Self { ... }
    
    pub fn data<T: Voxel + Encoding>(mut self, data: Vec<T>) -> Self { ... }
    
    pub fn build(self) -> Result<MrcWriter, MrcError> { ... }
}

impl MrcWriter {
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), MrcError> { ... }
}
```

---

## Phase 5: Dynamic Dispatch

```rust
// src/dynamic.rs

/// Runtime type dispatch for unknown modes
pub enum VolumeData {
    I8(VecVolume<i8>),
    I16(VecVolume<i16>),
    U16(VecVolume<u16>),
    F32(VecVolume<f32>),
    F16(VecVolume<half::f16>),
    ComplexI16(VecVolume<Complex<i16>>),
    ComplexF32(VecVolume<Complex<f32>>),
    U8(VecVolume<u8>),  // Packed4Bit decoded
}

impl VolumeData {
    pub fn shape(&self) -> [usize; 3] { ... }
    pub fn mode(&self) -> Mode { ... }
    
    /// Try to get as specific type
    pub fn as_f32(&self) -> Option<&VecVolume<f32>> { ... }
}
```

---

## Migration Strategy

### Step 1: Create New Types (Non-Breaking)
- Add `src/header/` module with RawHeader + Header
- Add `src/voxel/` module with traits
- Add `src/encoding.rs` with trait
- Add `src/storage/` module with Storage trait
- Add `src/volume/` module with Volume<T, S, D>
- Add `src/dynamic.rs` with VolumeData enum

### Step 2: Add IO Layer
- Add `src/io/reader.rs` with MrcReader
- Add `src/io/writer.rs` with MrcWriter + builder
- Keep MrcFile/MrcMmap working with deprecation warnings

### Step 3: Deprecation
- Mark DataBlock/DataBlockMut as deprecated
- Mark MrcView/MrcViewMut as deprecated
- Add conversion functions: `MrcView::into_volume()`

### Step 4: Update Tests
- Convert tests to use new API
- Add new tests for Volume<T, S, D>

### Step 5: Remove Old Code
- Remove deprecated types
- Clean up module structure

---

## Feature Flags

```toml
[features]
default = ["std", "mmap"]
std = []
mmap = ["std", "dep:memmap2"]
f16 = ["dep:half"]
rayon = ["dep:rayon"]
ndarray = ["dep:ndarray"]
fft = ["dep:rustfft"]
```

---

## Rust 2024 Features Used

1. **const generics** - `Volume<T, S, const D: usize>` for dimensionality
2. **GATs** - `Storage<T>` with associated types
3. **impl Trait in trait** - iterators returning `impl Iterator`
4. **let chains** - cleaner pattern matching in validation
5. **inline const** - `const { ... }` in functions

---

## API Comparison

### Before (Current)
```rust
let file = MrcFile::open("map.mrc")?;
let view = file.read_view()?;
let floats = view.data().to_vec_f32()?;  // Allocates
for val in floats { ... }
```

### After (Proposed)
```rust
let reader = MrcReader::open("map.mrc")?;
let volume = reader.read_volume::<f32>()?;  // Zero-copy decode
for val in volume.iter() { ... }  // Lazy, no allocation
```

### Writing (Current)
```rust
let mut header = Header::builder()
    .dimensions(64, 64, 64)
    .mode(Mode::Float32)
    .build();
let file = MrcFile::create("out.mrc", header)?;
file.write_data(&data)?;
```

### Writing (Proposed)
```rust
MrcWriter::builder()
    .shape(64, 64, 64)
    .mode(Mode::Float32)
    .data(data)?
    .write("out.mrc")?;
```

---

## Benefits

| Aspect | Current | Proposed |
|--------|---------|----------|
| Type Safety | Runtime | Compile-time |
| Mode Dispatch | Every access | Once at read |
| Zero-copy | Limited | Full support |
| SIMD-friendly | No | Yes |
| Parallel Iter | Manual | Built-in |
| Streaming | No | ChunkStorage |
| API Simplicity | DataBlock methods | Volume<T> methods |

---

## Estimated Effort

| Phase | Files Changed | Lines Changed | Risk |
|-------|---------------|---------------|------|
| Phase 1: Traits | 5 new | ~300 | Low |
| Phase 2: Header | 3 new | ~200 | Low |
| Phase 3: Volume | 4 new | ~400 | Medium |
| Phase 4: IO | 3 new | ~300 | Medium |
| Phase 5: Dynamic | 1 new | ~100 | Low |
| Phase 6: Cleanup | 10 modify | ~500 | High |

**Total**: ~1800 new lines, ~500 modified/deleted

---

