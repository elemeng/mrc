# Code Review: `mrc` v0.2.3

**Date:** 2026-05-09
**Reviewer:** Kimi Code CLI
**Scope:** Full codebase review (`src/`, `Cargo.toml`, CI, documentation)

---

## Overview

A Rust library for reading/writing MRC-2014 files (cryo-EM/microscopy). The crate targets `no_std` with optional `std`, SIMD acceleration, memory-mapped I/O, and type conversion pipelines. The architecture is generally clean with good separation between header parsing, endian codecs, conversion, and I/O.

---

## 🔴 Critical Issues

### 1. Tests do not compile

```rust
// src/engine/convert.rs:1006
assert_eq!(c.to_real(ComplexToRealStrategy::RealPart), 3.0);
// Error: ComplexToRealStrategy is not in scope in the test module
```

**Fix:** Add `use crate::ComplexToRealStrategy;` to the `#[cfg(test)]` module.

### 2. `no_std` support is broken

`extern crate alloc;` is gated behind `#[cfg(feature = "std")]`, which means `--no-default-features` fails immediately because `Vec`, `String`, etc. are unavailable. Additionally, `decode_slice` and `encode_slice` are `#[cfg(feature = "std")]` gated, but `Reader::decode_block` (which is not `std`-gated) depends on them.

**Fix:** Move `extern crate alloc;` out of the `std` cfg gate. Either make `alloc` unconditional or add an `alloc` feature.

### 3. `Writer::finalize` silently drops data when parallel writes are mixed

```rust
#[cfg(feature = "parallel")]
{
    if !self.parallel_writes {
        // Only writes buffered data if NO parallel writes were used
        self.file.write_all(&self.data)...
    }
}
```

If any `write_block_parallel` call is made, all data previously written via `write_block` (stored in `self.data`) is **never flushed to disk**.

**Fix:** Always write `self.data` in `finalize`, or redesign `Writer` to avoid dual buffers.

---

## 🟡 Bugs

### 4. `SlabIter::nth` (and all slab iterator variants) has incorrect semantics

```rust
// SlabIter::nth
fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.z = n;   // BUG: should be n * self.slab_size
    self.next()
}
```

With `slab_size = 10`, `nth(1)` returns the slab starting at Z=1 instead of Z=10. The same bug exists in `MmapSlabIter`, `SlabIterConverted`, and `MmapSlabIterConverted`.

### 5. Error messages discard underlying I/O details

Throughout the codebase:

```rust
.map_err(|_| Error::Io("open file".into()))
```

This throws away `std::io::Error` details (OS error codes, paths, etc.). Users will see `"IO error: open file"` instead of `"No such file or directory (os error 2)"`.

**Fix:** Include the source error: `Error::Io(e.to_string())` or better yet, make `Error::Io` wrap `std::io::Error` directly.

---

## 🟠 Design & API Concerns

### 6. `Writer` always buffers the entire file in RAM

```rust
let data_size = header.data_size();
let data = alloc::vec![0u8; data_size];
```

For a `[2048, 2048, 512]` f32 volume this is ~8.5 GB of RAM *before* writing. The README claims "zero-allocation" but this is allocation-heavy for writers.

**Recommendation:** Stream writes directly to the file, or document that `Writer` is for small-to-medium files and `MmapWriter` is required for large volumes.

### 7. `SliceAccess` trait is unsound

```rust
fn slice_mut<T: EndianCodec>(&mut self, z: usize) -> Result<&mut [T], Error> {
    // ...
    unsafe {
        let ptr = bytes.as_mut_ptr() as *mut T;
        Ok(core::slice::from_raw_parts_mut(ptr, nx * ny))
    }
}
```

There is no validation that `T::BYTE_SIZE == self.bytes_per_voxel` or that `T` matches the file's `Mode`. A caller can do `writer.slice_mut::<f32>(0)` on an `i16` file and get silent data corruption.

**Recommendation:** Add a runtime check: `assert_eq!(core::mem::size_of::<T>(), self.bytes_per_voxel)`.

### 8. `decode_block_zero_copy` is not zero-copy

It `copy_nonoverlapping`s bytes into a newly allocated `Vec<T>`. The only true zero-copy path is `MmapReader::data_bytes()`, which returns a `&[u8]`. The naming is misleading.

### 9. `encode_block_parallel` thread-local buffer is ineffective

```rust
ENCODE_BUFFER.with(|buf| {
    let mut buffer = buf.borrow_mut();
    // ...
    (chunk_idx, buffer.clone())  // clones every chunk!
})
```

The thread-local buffer exists to avoid allocation, but `buffer.clone()` allocates a new `Vec` for every chunk anyway.

### 10. `std::eprintln!` in library code

`Writer::create` and `MmapWriter::create` print warnings to stderr. Libraries should not write to stderr; they should return warnings or use a logging facade.

---

## 🟢 Positive Aspects

- **Clean pipeline architecture:** The 4-layer pipeline (Raw Bytes → Endian → Typed → Converted) is well-documented and consistently applied.
- **Good SIMD coverage:** AVX2 and NEON kernels for `i8→f32`, `i16→f32`, `u16→f32`, `u8→f32` with scalar fallbacks.
- **MRC2014 compliance:** Header validation is thorough (ISPG ranges, axis permutation checks, legacy MAP variants).
- **Iterator-centric API:** `slices()`, `slabs()`, `blocks()` provide ergonomic streaming access.
- **Feature flags are granular:** `std`, `mmap`, `simd`, `parallel`, `f16` allow users to trim dependencies.

---

## 📋 Recommendations (by priority)

| Priority | Item |
|----------|------|
| **P0** | Fix test compilation (`ComplexToRealStrategy` import) |
| **P0** | Fix `no_std` / `alloc` feature gating |
| **P0** | Fix `Writer::finalize` parallel-write data loss |
| **P1** | Fix `SlabIter::*::nth` semantics |
| **P1** | Preserve `std::io::Error` details in `Error::Io` |
| **P1** | Validate `T` size in `SliceAccess::slice_mut` |
| **P2** | Remove or use `once_cell` dependency (currently unused) |
| **P2** | Remove `std::eprintln!` from library code |
| **P2** | Update `CONVERSION_GAPS.md` (M101 unpacking, `ComplexToRealStrategy`, `CheckedConvert` are already implemented) |
| **P2** | Add integration tests (`tests/` directory) — currently only inline unit tests exist |
| **P3** | Add `try_new` to `VoxelBlock` to avoid panics on shape mismatch |
| **P3** | CI only runs `cargo test --verbose` (no `--all-features`), so SIMD/parallel code is not tested in CI |

---

## Summary

The codebase shows strong architectural thinking and good domain knowledge of the MRC format, but it has **three critical issues** (broken tests, broken `no_std`, and silent data loss on mixed parallel writes) that should be fixed before the next release. The `SlabIter::nth` bug and poor I/O error reporting are also notable. Once these are addressed, the foundation is solid for the v0.3 roadmap items.
