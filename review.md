# Code Review: mrc crate

**Focus:** Correctness, Performance, Redundancy  
**Commit:** HEAD (working tree)

---

## 🔴 Correctness

### 1. `read_block_bytes` is broken for non-slab blocks
**Severity: Critical**

`validate_block_read` (and all `read_block_bytes` implementations) treats any `[ox, oy, oz] + [sx, sy, sz]` as a **single contiguous byte range**:

```rust
let linear = volume_shape.checked_linear_index(offset)?; // ox + oy*nx + oz*nx*ny
let start_byte = linear * mode.byte_size();
// ... returns self.data[start_byte..end_byte]
```

This is only correct when `ox == 0 && sx == nx` (full rows) and `oy == 0 && sy == ny` (full slices).  
If `ox > 0` or `sx < nx`, the data in the file is **not contiguous** — rows are padded to `nx` voxels.

**Impact:** `BlockIter` (which tiles with arbitrary `block_shape`) and any manual `read_block` with a sub-XY block will return **garbage data** (voxels from the next row(s) in the file).

**Fix:** Implement gather/scatter for non-contiguous blocks, or restrict `read_block_bytes` to blocks where `ox == 0 && sx == nx && sy == ny` (slabs). `BlockIter` would need to collect row-by-row.

---

### 2. `MmapWriter::slice` / `slice_mut` ignore file endianness
**Severity: High**

```rust
pub fn slice<T: EndianCodec>(&self, z: usize) -> Result<&[T], Error> {
    // ...
    unsafe {
        let ptr = bytes.as_ptr() as *const T;
        Ok(core::slice::from_raw_parts(ptr, nx * ny))
    }
}
```

These methods return raw mmap bytes reinterpreted as `&[T]`. If the file endianness differs from the host, the values are wrong. While the crate writes LE by default, a user could open an existing BE file with `MmapWriter` (it's technically possible via `MmapWriter::create` with a pre-built header) or run on a big-endian host.

**Fix:** Either refuse to return slices when `endian != FileEndian::native()`, or document the precondition loudly. Returning `Err` is safer.

---

### 3. `Header::add_label` does not clear old label bytes
**Severity: Medium**

```rust
let start = slot * 80;
self.label[start..start + len].copy_from_slice(bytes);
```

When overwriting a slot (including after FIFO shift), only `len` bytes are written. The remaining `80 - len` bytes of the previous label are left intact. This produces corrupted/padded labels.

**Fix:** Zero the full 80-byte slot before `copy_from_slice`:

```rust
self.label[start..start + 80].fill(b' ');
self.label[start..start + len].copy_from_slice(bytes);
```

---

### 4. `decode_slice` silently ignores trailing bytes
**Severity: Medium**

```rust
let n = bytes.len() / T::BYTE_SIZE; // integer division truncates
```

If `bytes.len()` is not an exact multiple of `T::BYTE_SIZE`, the remainder is silently dropped. This could mask file corruption or miscounted blocks.

**Fix:** `assert_eq!(bytes.len() % T::BYTE_SIZE, 0)` or return an error.

---

### 5. `stats_real` computes mean in `f32` then casts to `f64` for variance
**Severity: Low–Medium**

```rust
let mean = (sum / data.len() as f64) as f32; // precision loss
let variance = iter().map(|v| {
    let d = v - mean as f64; // double-cast
    d * d
}).sum::<f64>() / data.len() as f64;
```

The mean is rounded to `f32`, then promoted back to `f64` for the variance pass. For large datasets this loses precision and can produce a slightly wrong RMS.

**Fix:** Keep `mean` as `f64` throughout.

---

### 6. `Header::data_size()` returns `Some(0)` for unknown modes
**Severity: Low–Medium**

```rust
None => Some(0), // unknown/unsupported
```

Callers (e.g. `Reader::_open`) interpret `Some(0)` as "empty data" rather than "unsupported mode". A file with an invalid mode code will open successfully but have its data discarded without a clear error.

**Fix:** Return `None` or propagate `Error::UnsupportedMode`.

---

### 7. `update_header_stats_from_bytes` silently falls back to `Float32`
**Severity: Low**

```rust
let mode = Mode::from_i32(header.mode);
let (dmin, ...) = compute_stats(bytes, mode.unwrap_or(Mode::Float32), endian);
```

If `mode` is unknown, it interprets the raw bytes as `f32` statistics. This will produce nonsensical results.

**Fix:** Return early or propagate the error.

---

### 8. `Reader::_open` trailing-byte check reports misleading `actual` size
**Severity: Low**

```rust
if file.read(&mut trailing)? > 0 {
    return Err(Error::FileSizeMismatch {
        expected: header.data_offset() + data_size,
        actual: header.data_offset() + data_size + 1, // "at least 1 extra byte"
    });
}
```

The error says the actual size is exactly `expected + 1`, even if the file is 1 GB too large.

**Fix:** Use `file.metadata()` to get the real size, or report "at least 1 extra byte" in the message.

---

## 🟡 Performance

### 1. `stats_real` makes 4 passes over the data
```rust
let min = iter().fold(...);   // pass 1
let max = iter().fold(...);   // pass 2
let sum  = iter().sum();      // pass 3
let variance = iter().map(...).sum(); // pass 4
```

**Fix:** Fold into accumulators in a single pass. For `Float32Complex` / `Int16Complex`, the RMS functions also make 3 passes.

---

### 2. `decode_slice` / `encode_slice` branch on `endian` per element
```rust
fn from_bytes(..., endian: FileEndian) -> Self {
    match endian { FileEndian::LittleEndian => ..., BigEndian => ... }
}
```

The `match` is evaluated for **every single voxel**. The compiler may hoist it, but it's not guaranteed — especially across the `par_chunks` boundary.

**Fix:** Hoist the endian branch outside the loop:

```rust
if endian == FileEndian::native() {
    // fast memcpy path
} else {
    // single-endian swap loop
}
```

This is already done for `decode_native_endian`, but the non-native path still branches per element.

---

### 3. `decode_slice` initializes memory with `T::default()` before overwriting
```rust
let mut result = Vec::with_capacity(n);
result.resize(n, T::default()); // writes n times
// then overwrites all n elements
```

For large blocks this is pure overhead.

**Fix:** Use `unsafe { result.set_len(n) }` (all elements are overwritten by the loop) or `spare_capacity_mut()` + `MaybeUninit` if you want to avoid `unsafe`.

---

### 4. `CompressedWriter::write_block` double-buffers
```rust
let mut buffer = vec![0u8; byte_len]; // alloc #1
encode_slice(&block.data, &mut buffer, file_endian);
self.data[start_byte..start_byte + byte_len].copy_from_slice(&buffer); // copy
```

`self.data` is already a `Vec<u8>`. Encode directly into the target sub-slice:

```rust
let dst = &mut self.data[start_byte..start_byte + byte_len];
encode_slice(&block.data, dst, file_endian);
```

---

### 5. SIMD `set_len` dance is unnecessarily expensive
The SIMD kernels call `dst.set_len(i)` **inside** the hot loop on every iteration:

```rust
while i + 32 <= src.len() {
    // ...
    dst.set_len(i);
    _mm256_storeu_ps(dst.as_mut_ptr().add(i), ...);
    i += 32;
}
```

`set_len` writes the length field of the Vec each time. On modern CPUs this is cheap, but it pollutes the store buffer and prevents the compiler from hoisting the pointer.

**Fix:** Keep the Vec at full capacity (`set_len(src.len())` once before the loop) and store through a raw pointer that you bump. Or use `std::simd` portable vectors for cleaner code (Rust 1.85+).

---

### 6. `MrcReader` enum dispatch overhead
Every `MrcReader` method is a `match` over variants. For iterator-heavy workloads, the dynamic dispatch adds branch overhead per call. The `slices_f32` API already boxes the iterator (`Box<dyn Iterator>`), which adds a vtable dereference per `next()`.

**Mitigation:** This is a design trade-off for ergonomics. Not a bug, but worth documenting that power users should use the concrete types directly.

---

## 🟢 Redundancy / Maintainability

### 1. Header parsing logic is copy-pasted in 4 places
`Reader::_open`, `MmapReader::_open`, `Reader::_open_gzip`, and `Reader::_open_bzip2` all contain ~40 lines of identical header validation and buffer splitting.

**Fix:** Extract a shared helper:

```rust
fn open_from_bytes(
    buf: &[u8],
    permissive: bool,
) -> Result<(Header, Vec<u8>, Vec<u8>, Vec<String>), Error>
```

---

### 2. `Reader` and `MmapReader` duplicate convenience iterators
`slices_f32`, `slabs_f32`, `slices_u8`, `slices_mode0`, `slabs_mode0` are implemented separately on both types with nearly identical bodies. `reader_common.rs` already centralizes `slices_f32` and `slabs_f32`; the others should follow.

---

### 3. `WriterBuilder` and `MmapWriterBuilder` are nearly identical
Both have `shape`, `mode`, `cell_lengths`, `ispg`, `exttyp`, `nsymbt`, `origin`.

**Fix:** A single generic builder (`Builder<W: WriterTarget>`) or a shared `HeaderConfig` struct would halve the code.

---

### 4. `Writer`, `MmapWriter`, `CompressedWriter` duplicate validation
Every `write_block` repeats:

```rust
if T::MODE != self.mode() { return Err(Error::ModeMismatch { ... }); }
if !self.shape.contains_block(...) { return Err(Error::BoundsError); }
```

**Fix:** A `fn validate_write<T: Voxel>(&self, block: &VoxelBlock<T>) -> Result<(), Error>` on a shared trait or macro would deduplicate this.

---

### 5. `GzipReader` / `Bzip2Reader` boilerplate
The two types are structurally identical (tuple struct + `Deref` + `DerefMut` + `open` / `open_permissive`). The `_open_gzip` and `_open_bzip2` methods on `Reader` are also almost identical except for the decoder type.

**Fix:** A small macro or a generic `CompressedReader<D: Read>` wrapper would eliminate the duplication.

---

### 6. `mode()` accessor is repeated 6+ times
Every reader and writer has:

```rust
pub fn mode(&self) -> Mode {
    Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
}
```

**Fix:** Add an inherent method on `Header` or a single trait.

---

## 🟣 Minor Nitpicks

- **`FileEndian::from_machst_with_info` prints to `stderr`**. Libraries should not write to stderr; return the warning in `MachstInfo`.
- **`fei.rs` uses `le_u32` for `bitmask_1` while all other fields are BE**. If intentional, document it; if not, fix it.
- **`lib.rs` `feature(f16)` may be unnecessary** on Rust ≥ 1.85 (check stability on your target tier).
- **`BlockIter` is missing `ExactSizeIterator`**. The remaining count is `ceil((nx-px)/cx) * ceil((ny-py)/cy) * ceil((nz-pz)/cz)`, which can be precomputed or calculated from state.

---

## Summary Table

| # | Issue | Severity | File(s) |
|---|-------|----------|---------|
| 1 | `read_block_bytes` assumes contiguous layout for arbitrary blocks | **Critical** | `iter.rs`, `reader_common.rs`, all readers |
| 2 | `MmapWriter::slice` ignores endianness | **High** | `writer.rs` |
| 3 | `add_label` doesn't clear old bytes | **Medium** | `header.rs` |
| 4 | `decode_slice` silently ignores trailing bytes | **Medium** | `engine/codec.rs` |
| 5 | `stats_real` 4-pass + precision loss | **Low–Med** | `engine/stats.rs` |
| 6 | `data_size()` returns `Some(0)` for bad modes | **Low–Med** | `header.rs` |
| 7 | `update_header_stats_from_bytes` falls back to Float32 | **Low** | `writer.rs` |
| 8 | `Reader::_open` misleading file-size error | **Low** | `io/buffered.rs` |
| 9 | Header parsing duplicated 4× | Maintainability | `io/*` |
| 10 | Convenience iterators duplicated across readers | Maintainability | `io/buffered.rs`, `io/mmap_reader.rs` |
| 11 | `CompressedWriter::write_block` double-buffers | Perf | `io/writer.rs` |
| 12 | SIMD kernels call `set_len` in hot loop | Perf | `engine/simd.rs` |
| 13 | `encode_slice` branches on endian per element | Perf | `engine/codec.rs` |
