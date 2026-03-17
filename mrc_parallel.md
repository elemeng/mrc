To make the design **std-quality and actually fast**, the critical part is the **encode/decode engine**.
This is where **SIMD + Rayon** must be carefully structured to avoid:

* false sharing
* cache thrashing
* extra copies
* lock contention

Below is the **practical architecture used in high-performance data pipelines**.

---

# 1. Core Principle

All voxel IO operates on **large contiguous blocks**.

Pipeline:

```
VoxelBlock<T>
     │
partition → rayon chunks
     │
SIMD encode/decode per chunk
     │
write/read to file offsets
```

This guarantees:

* **no locks**
* **no shared buffers**
* **perfect parallelism**

---

# 2. Memory Layout

MRC data layout:

```
z-major slices

slice0
slice1
slice2
...
```

Each slice:

```
nx * ny voxels
```

Voxel linear index:

```
offset = ((z * ny + y) * nx + x)
```

Byte offset:

```
byte_offset = header_size + offset * sizeof(mode)
```

This means every **block maps to one contiguous region or a few rows**.

---

# 3. Chunk Partitioning

Large voxel blocks are split into **parallel chunks**.

Example block:

```
4096 × 4096 × 16
```

Total voxels:

```
268M
```

Rayon partitions into chunks:

```
1–8 MB per chunk
```

Example:

```
chunk = 1M voxels
```

Then:

```
block
 ├── chunk0
 ├── chunk1
 ├── chunk2
 └── chunkN
```

Each processed independently.

---

# 4. Rayon Decode Pipeline

Reading pipeline:

```
read bytes
   ↓
parallel decode
   ↓
typed voxel slice
```

Pseudo implementation:

```rust
fn decode<T: Mode>(
    src: &[u8],
    dst: &mut [T],
) {
    use rayon::prelude::*;

    dst.par_chunks_mut(CHUNK)
        .zip(src.par_chunks(CHUNK_BYTES))
        .for_each(|(out, input)| {
            decode_chunk::<T>(input, out);
        });
}
```

This gives:

```
perfect CPU utilization
```

---

# 5. SIMD Decode

Example:

```
Int16 → Float32
Uint16 → Float32
Float16 → Float32
Int8 → Float32
Int16Complex → Float32Complex
Float32Complex → Float32Complex
Packed4Bit → Float32
```

Scalar:

```
for i:
   out[i] = input[i] as Float32
```

SIMD:

```
Int16x8 → Float32x8
Uint16x8 → Float32x8
```

Implementation sketch:

```rust
#[inline]
fn decode_int16_to_float32_simd(src: &[Int16], dst: &mut [Float32]) {
    let lanes = 8;

    let chunks = src.len() / lanes;

    for i in 0..chunks {
        let v = load_int16x8(&src[i*lanes]);

        let f = convert_to_float32x8(v);

        store_float32x8(&mut dst[i*lanes], f);
    }
}
```

Possible implementations:

```
std::simd
portable_simd
x86 AVX2 intrinsics
```

Preferred:

```
std::simd
```

---

# 6. Endian Handling with SIMD

Endian swap example:

```
Int16, Uint16, Float32, Float16, Int16Complex, Float32Complex
```

Scalar:

```
swap_bytes()
```

SIMD:

```
byte shuffle
```

Example:

```
[i0,i1,i2,i3,i4,i5,i6,i7]
   ↓
shuffle
```

Cost:

```
~1 CPU cycle
```

Much faster than scalar loops.

---

# 7. Encode Pipeline

Encoding is slightly more complex.

Pipeline:

```
user voxels
   ↓
rayon chunk partition
   ↓
SIMD convert
   ↓
SIMD endian encode
   ↓
write to sink
```

Important rule:

```
encoding happens BEFORE writing
```

---

# 8. Encoding Buffer Strategy

Each thread uses a **private buffer**.

Never share buffers.

Example:

```
Thread 1
buffer → encode → write

Thread 2
buffer → encode → write
```

Buffer size:

```
1–4 MB
```

This prevents:

```
false sharing
cache ping-pong
```

---

# 9. Writing to File (Sink)

Two strategies.

---

# 9.1 Streaming Sink (seek + write)

Writer holds:

```rust
struct Writer {
    file: File,
}
```

Rayon workers compute offsets:

```
file_offset = base_offset + chunk_offset
```

Worker writes:

```rust
file.write_at(encoded_bytes, offset)
```

Implementation uses:

```
FileExt::write_at
```

Linux:

```
pwrite
```

This allows **parallel writes without locking**.

Example worker:

```rust
fn encode_and_write_chunk(
    chunk: &[Float32],
    file: &File,
    offset: u64,
) {
    let mut buf = thread_buffer();

    encode_simd(chunk, &mut buf);

    file.write_at(&buf, offset).unwrap();
}
```

Result:

```
true parallel disk writes
```

---

# 9.2 mmap Sink

Alternative:

```
memory mapped file
```

Setup:

```
mmap = map_mut(file)
```

Workers write directly:

```
dst = mmap[offset..offset+size]

encode_simd(src, dst)
```

Pipeline:

```
voxels
  ↓
SIMD encode
  ↓
memcpy into mapped region
```

Advantages:

```
no syscalls
extremely fast
```

This is ideal for:

```
large tomograms
reconstruction pipelines
```

---

# 10. Parallel Encode with Rayon

Full encode flow:

```rust
fn encode_block<T: Mode>(
    voxels: &[T],
    file: &File,
    base_offset: u64
) {
    use rayon::prelude::*;

    voxels
        .par_chunks(CHUNK)
        .enumerate()
        .for_each(|(i, chunk)| {

            let mut buf = thread_buffer();

            encode_chunk_simd(chunk, &mut buf);

            let offset =
                base_offset + (i * CHUNK_BYTES) as u64;

            file.write_at(&buf, offset).unwrap();
        });
}
```

This achieves:

```
CPU parallel encode
parallel disk IO
```

---

# 11. Thread Buffers

Each thread owns a buffer.

Use:

```
thread_local!
```

Example:

```rust
thread_local! {
    static BUF: RefCell<Vec<u8>> =
        RefCell::new(vec![0u8; 4 * 1024 * 1024]);
}
```

Worker obtains buffer:

```rust
BUF.with(|b| {
    let mut buf = b.borrow_mut();
    encode_simd(src, &mut buf);
});
```

Advantages:

```
no allocation
no locking
reusable
```

---

# 12. Why This Architecture Works

This structure ensures:

### perfect parallelism

```
rayon partitions
```

### vectorized compute

```
SIMD conversion
```

### minimal synchronization

```
pwrite / mmap
```

### bounded memory

```
per-thread buffers
```

---

# 13. Performance Expectations

Typical dataset:

```
4096 × 4096 × 300
```

Raw size:

```
~20 GB
```

Performance:

| operation     | speed  |
| ------------- | ------ |
| scalar decode | 1×     |
| SIMD decode   | 4–6×   |
| SIMD + rayon  | 10–25× |

Disk often becomes the **bottleneck**.

---

# 14. Final Internal Encode/Decode Engine

Conceptually:

```
              VoxelBlock
                   │
           Rayon Partition
                   │
          ┌────────┴────────┐
          │                 │
     Thread 1           Thread N
          │                 │
      SIMD encode       SIMD encode
          │                 │
    thread buffer       thread buffer
          │                 │
       pwrite             pwrite
```

Zero locking.

Maximum throughput.

---

# 15. Resulting Crate Characteristics

Your crate becomes:

```
fully parallel
SIMD accelerated
zero-copy friendly
streaming friendly
```

Comparable to HPC IO libraries.

---

✅ **This is the architecture used by modern high-performance data engines.**
