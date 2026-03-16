Update **comprehensive design document** for a **Rust std-quality MRC crate**:

* **Iterator-centric reading**
* **Voxel-block writing**
* **SIMD + Rayon internal acceleration**
* **Zero-copy mmap paths**
* **Minimal API surface**
* **Pipeline-agnostic**

This design intentionally separates **volume semantics (reader)** from **voxel storage (writer)**.

---

# Rust MRC Crate — Final Design

## 1. Design Goals

The crate is designed as a **high-performance volumetric IO engine** for cryo-EM / cryo-ET workflows.

Primary goals:

```text
minimal API surface
iterator-centric reading
block-based writing
pipeline-agnostic
SIMD accelerated
multi-core encoding/decoding
zero-copy where possible
std-quality Rust design
```

The crate **does not implement processing pipelines**.
It only provides **efficient access to voxel data**.

---

# 2. Design Philosophy

The library follows **three core principles**.

## 2.1 Reader: Semantic Iteration

The file layout is fixed, therefore the reader can expose **semantic iteration**:

```text
slice iteration
slab iteration
block iteration
```

These are convenience abstractions.

Internally all reduce to:

```text
read_voxels(offset, shape)
```

---

## 2.2 Writer: Raw Voxel Placement

Writers cannot assume semantics.

Data producers may generate:

```text
slices
multi-slice slabs
GPU tiles
FFT blocks
AI inference chunks
```

Therefore the writer only understands:

```text
voxels + placement
```

Primitive operation:

```text
write_voxels(offset, shape, data)
```

---

## 2.3 Internal Acceleration

Encoding and decoding are automatically accelerated using:

```text
SIMD vectorization
Rayon parallelism
```

These are **internal implementation details**, not user features.

Users do not configure them.

---

# 3. Core Data Model

## 3.1 Volume Geometry

Every MRC volume is defined by:

```rust
struct VolumeShape {
    nx: usize,
    ny: usize,
    nz: usize,
}
```

---

## 3.2 Voxel Mode

MRC mode defines the stored type.

```rust
enum Mode {
    Int8,
    Int16,
    UInt16,
    Float16,
    Float32,
}
```

Each mode maps to a Rust type.

---

## 3.3 VoxelBlock

Universal representation of voxel chunks.

```rust
pub struct VoxelBlock<'a, T> {
    pub offset: [usize; 3],
    pub shape:  [usize; 3],
    pub data:   &'a [T],
}
```

This supports:

```text
slice
slab
tile
full volume
```

---

# 4. Reader Architecture

## 4.1 Opening Files

```rust
let mrc = mrc::open("volume.mrc")?;
```

Reader loads:

```text
header
volume shape
mode
endian
data offset
```

---

## 4.2 Typed Decoding

Voxel type is specified by the user:

```rust
mrc.slices::<f32>()
```

If file mode differs, automatic conversion occurs.

---

# 5. Reader API

## 5.1 Slice Iterator

Most common access pattern.

```rust
for slice in mrc.slices::<f32>() {
    process(slice);
}
```

Slice shape:

```text
[nx, ny]
```

---

## 5.2 Slab Iterator

Efficient multi-slice processing.

```rust
for slab in mrc.slabs::<f32>(16) {
    process_slab(slab);
}
```

Slab shape:

```text
[nx, ny, 16]
```

---

## 5.3 Block Iterator

Arbitrary chunking.

```rust
for block in mrc.blocks::<f32>([256,256,16]) {
    process(block);
}
```

Block iteration supports:

```text
tiling
GPU processing
FFT pipelines
```

---

# 6. Iterator Engine

All iterators share the same engine.

Internal state:

```rust
struct IterEngine {
    reader
    index
    chunk_shape
    decode_fn
}
```

Iterator performs:

```text
compute file offset
read bytes
decode voxels
yield typed view
```

---

# 7. Random Slice Access

Accessing a specific slice:

```rust
let slice = mrc.slices::<f32>().nth(29).unwrap();
```

Internally optimized by overriding `.nth()`:

```text
seek → decode → return
```

Complexity:

```text
O(1)
```

---

# 8. Decoding Pipeline

Decode process:

```text
disk bytes
   ↓
SIMD endian swap
   ↓
SIMD type conversion
   ↓
Rayon parallel chunk processing
   ↓
typed voxel slice
```

Example conversion:

```text
i16 → f32
```

Vectorized using SIMD.

---

# 9. Writer Architecture

## 9.1 Creating Files

```rust
let mut writer = mrc::create("output.mrc")
    .shape([nx,ny,nz])
    .mode::<f32>()
    .finish()?;
```

The writer:

```text
writes header placeholder
allocates data region
initializes encoding pipeline
```

---

# 10. Writer API

## 10.1 Write Voxel Blocks

Single universal write operation.

```rust
writer.write_block(block)?;
```

Example slice:

```rust
writer.write_block(VoxelBlock {
    offset: [0,0,z],
    shape:  [nx,ny,1],
    data:   &slice,
});
```

---

## 10.2 Slab Writing

```rust
writer.write_block(VoxelBlock {
    offset: [0,0,z],
    shape:  [nx,ny,16],
    data:   &slab,
});
```

---

## 10.3 Tile Writing

```rust
writer.write_block(VoxelBlock {
    offset: [x,y,z],
    shape:  [tx,ty,tz],
    data:   &tile,
});
```

---

# 11. Encoding Pipeline

Encoding process:

```text
user voxels
   ↓
SIMD type conversion
   ↓
SIMD endian encoding
   ↓
Rayon parallel chunk encoding
   ↓
file storage
```

Example conversion:

```text
f32 → i16
```

---

# 12. Writer Backends

Two implementations.

---

## 12.1 Streaming Writer

Uses:

```text
seek + write
```

Best for:

```text
slice streaming
network pipelines
low memory systems
```

---

## 12.2 mmap Writer

Maps entire file into memory.

```text
memcpy → mapped file region
```

Best for:

```text
large tomograms
parallel reconstruction
direct slice writes
```

Example:

```rust
let slice = writer.slice_mut(z);
reconstruct_into(slice);
```

---

# 13. Header Finalization

Statistics are accumulated while writing:

```text
min
max
mean
```

After final block:

```text
seek(0)
rewrite header
```

---

# 14. Parallel Processing Model

Encoding and decoding are internally parallel.

Example decode flow:

```text
voxel block
    ↓
rayon chunk partition
    ↓
SIMD conversion per chunk
```

All CPU cores utilized.

---

# 15. Performance Characteristics

For large datasets:

Example stack:

```text
4096 × 4096 × 300
```

Expected improvements:

| optimization      | speedup          |
| ----------------- | ---------------- |
| SIMD decoding     | 4×               |
| Rayon parallelism | up to core count |
| Combined          | ~10–20×          |

---

# 16. Memory Strategy

Large volumes use streaming.

Memory usage remains bounded:

```text
iterator buffer
decode buffer
optional slab buffer
```

No full-volume allocation.

---

# 17. Error Handling

Typical errors:

```text
invalid header
unsupported mode
block out of bounds
type mismatch
```

Represented as:

```rust
enum Error {
    Io,
    InvalidHeader,
    UnsupportedMode,
    BoundsError,
}
```

---

# 18. Module Layout

Recommended internal structure:

```text
mrc
 ├── header
 ├── mode
 ├── reader
 ├── writer
 ├── iter
 ├── block
 ├── decode
 ├── encode
 ├── simd
 ├── io
 └── error
```

Each module remains small and focused.

---

# 19. Public API Summary

Reader:

```rust
mrc::open()

mrc.slices::<T>()
mrc.slabs::<T>(k)
mrc.blocks::<T>(shape)
```

Writer:

```rust
mrc::create()

writer.write_block()
writer.slice_mut()
```

Total public surface:

```text
~8 primary APIs
```

---

# 20. Final Architecture

Complete system:

```text
        Reader
           │
           ▼
  Iterator Engine
           │
   SIMD Decode + Rayon
           │
        Voxels
           │
           ▼
       Processing
           │
           ▼
      Voxel Blocks
           │
           ▼
       Writer
           │
   SIMD Encode + Rayon
           │
           ▼
        MRC File
```

---

# Final Design Summary

This crate provides:

```text
iterator-based reading
block-based writing
SIMD accelerated encoding/decoding
Rayon parallel processing
mmap zero-copy fast paths
pipeline-agnostic IO
minimal API surface
```

Conceptually it behaves like:

```text
std::io for volumetric data
```

optimized for **cryo-EM and tomography datasets**.
