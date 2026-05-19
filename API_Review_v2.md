# API Review v2 — Post-Conversion-Stripping

**Commit**: `7890df3`  
**Scope**: Core MRC I/O API (Reader, MmapReader, Writer, MmapWriter, iterators, errors)

---

## Summary

The stripped-down API is clean and focused. Most methods have clear contracts and the removal of the generic `Convert` trait matrix eliminated a major maintenance surface. However, a few **critical correctness bugs** remain in iterator adapters and writer endianness handling, plus several **ergonomic leaks** where implementation bounds (`Send + Default`) propagate into the public API.

---

## Critical Bugs

### 1. `Iterator::nth()` is implemented incorrectly in all slab/slice iterators

**All four iterators** set `self.z = n` (or `n * slab_size`) instead of *advancing* by `n` elements from the current position.

```rust
// iter.rs — SliceIter::nth (also in SlabIter, MmapSliceIter, MmapSlabIter)
fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.z = n;        // BUG: jumps to absolute index, ignores current position
    self.next()
}
```

Per `std::iter::Iterator` docs, `nth(n)` must consume `n` elements and return the next one. Repeated calls to `nth(0)` after some consumption must yield the next element, not restart from the beginning.

**Fix**: `self.z += n` (SliceIter), `self.z += n * self.slab_size` (SlabIter).

---

### 2. Writer hardcodes LittleEndian encoding regardless of header's `machst`

`Writer::write_block`, `MmapWriter::write_block`, and parallel variants all pass `FileEndian::LittleEndian` to `encode_slice`. If a user supplies a header with big-endian `machst`, the file header will claim big-endian while the voxel data is written little-endian.

```rust
// writer.rs line 132
encode_slice(&block.data, &mut buffer, FileEndian::LittleEndian);
```

**Options**:
- **A** (recommended): Force the header to little-endian in `Writer::create` / `MmapWriter::create` and document that new files are always LE.
- **B**: Encode data using `self.header.detect_endian()`.

Option A is simpler and matches the documented crate policy.

---

### 3. `Packed4Bit` block I/O computes wrong byte sizes

`Mode::Packed4Bit.byte_size()` returns `1` (the in-memory size of `Packed4Bit`), but on disk each voxel is 0.5 bytes (2 voxels per byte). `read_block_bytes` and `write_block` use `byte_size()` to compute:

```rust
let byte_len = sx * sy * sz * self.bytes_per_voxel; // = N * 1 for Packed4Bit
```

This overreads by 2× for Packed4Bit files. `Header::data_size()` has special handling (`n.div_ceil(2)`), but per-block I/O does not.

**Fix**: Add a helper like `mode.byte_size_for_count(n) -> usize` that returns `n.div_ceil(2)` for Packed4Bit, and use it in all block I/O math.

---

## API Design Issues

### 4. `slices<T>()`, `slabs<T>()`, `blocks<T>()` don't bound `T: Voxel` at call site

The bound is only on the `Iterator` impl, so `reader.slices::<String>()` compiles but the iterator is unusable. The error surfaces as a cryptic `ModeMismatch` at iteration time rather than a clear trait-bound error at the call site.

**Fix**: Add `T: Voxel` (or `T: EndianCodec + Voxel`) to the method signatures on `Reader` and `MmapReader`.

---

### 5. Unnecessary `Send + Default` bounds leak into public API

`read_block`, `slices`, `slabs`, `blocks`, and the iterator `next()` methods all require `T: Send + Copy + Default`. These bounds come from `decode_slice`'s parallel implementation (`par_chunks_mut` needs `Send`, `resize` needs `Default`). But:
- `Default` is not needed for the native-endian zero-copy path (`decode_block_zero_copy`).
- `Send` is irrelevant for sequential reads and non-parallel builds.

These bounds force users to implement `Default` and `Send` for custom voxel types even when they only use sequential APIs.

**Fix**: Remove `Default` and `Send` from `decode_block`, `read_block`, and iterator bounds. Keep them only inside `decode_slice` (which can add its own bounds via `where` clauses).

---

### 6. Missing `ExactSizeIterator` and `FusedIterator` impls

The slice/slab iterators have known remaining lengths but don't implement `ExactSizeIterator`. This prevents `.len()`, `.collect::<Vec<_>>().with_capacity()`, and other optimizations.

All iterators are fused (return `None` forever after exhaustion) but don't declare `FusedIterator`.

**Fix**: Add `impl ExactSizeIterator for SliceIter/SlabIter/MmapSliceIter/MmapSlabIter` and `impl FusedIterator` for all.

---

### 7. `HeaderBuilder::exttyp` returns `Result`, breaking builder chains

```rust
let header = HeaderBuilder::new()
    .shape([512, 512, 100])
    .exttyp("CCP4")?   // awkward ? in the middle
    .build()?;
```

Passing a non-4-char string is a programming error. Better to panic or accept `[u8; 4]`.

**Fix**: Change signature to `pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self`.

---

### 8. `Error` does not wrap `HeaderValidationError`

`HeaderBuilder::build()` returns `Result<Header, HeaderValidationError>`, but `WriterBuilder::finish()` returns `Result<Writer, Error>`. There is no conversion between the two, so users mixing both APIs must handle two error types.

**Fix**: Add `Error::InvalidHeaderDetailed(#[from] HeaderValidationError)` or have `WriterBuilder::finish()` propagate the detailed error.

---

### 9. Redundant `MmapWriterBuilder::new`

`MmapWriterBuilder` can only be constructed via `WriterBuilder::new(path).mmap()`, but it also has its own `new()` method. Two entry points for the same thing.

**Fix**: Remove `MmapWriterBuilder::new` (or make it `pub(crate)`).

---

## Minor Issues

### 10. `MmapReader::data_bytes()` can panic on truncated files

If the header claims more data than the file contains, `data_bytes()` slices past the end of the mmap and panics. `read_block_bytes` has a bounds check, but `data_bytes` does not.

**Fix**: Cap the slice at `self.mmap.len()` or return `Result<&[u8], Error>`.

### 11. `ComplexToRealStrategy` is unused by readers/writers

The enum is public and well-documented, but no reader method accepts it. Users with complex files must manually call `Float32Complex::to_real()`. Not a bug, but the API surface is slightly wider than the functionality.

### 12. Iterator type aliases not re-exported from `lib.rs`

`SliceIterF32` and `MmapSliceIterF32` exist but aren't in `pub use` statements. Users must fully qualify them or rely on type inference.

---

## Recommendations (Priority Order)

| # | Issue | Severity | Effort |
|---|-------|----------|--------|
| 1 | Fix `nth()` in all iterators | **Critical** | Low |
| 2 | Fix Writer endianness hardcoding | **Critical** | Low |
| 3 | Fix Packed4Bit block I/O | **Critical** | Medium |
| 4 | Add `T: Voxel` bound to `slices/slabs/blocks` | Medium | Low |
| 5 | Remove `Send + Default` from public API | Medium | Medium |
| 6 | Add `ExactSizeIterator` + `FusedIterator` | Low | Low |
| 7 | `HeaderBuilder::exttyp` signature | Low | Low |
| 8 | Unify `HeaderValidationError` into `Error` | Low | Low |
| 9 | Remove redundant `MmapWriterBuilder::new` | Low | Low |
