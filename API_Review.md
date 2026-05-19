# API Design Review: `mrc` v0.2.3

This review focuses on the **public API surface** — ergonomics, consistency, type safety, and discoverability — rather than implementation bugs (which are well-covered in `REVIEW.md`).

---

## ✅ Strengths

### Iterator-Centric Design
The `slices::<T>()`, `slabs::<T>(k)`, and `blocks::<T>(shape)` methods on both `Reader` and `MmapReader` provide a clean, composable streaming API. This is the right abstraction for large volumetric data.

```rust
for slice in reader.slices::<f32>() {
    let block = slice?;
}
```

### Type-Level Voxel Safety
The `Voxel` trait tying Rust types to `Mode` constants (`i16 → Mode::Int16`, etc.) enables compile-time dispatch for conversion pipelines. The `EndianCodec` trait is well-designed for extensibility.

### Feature Granularity
The feature flags (`mmap`, `simd`, `parallel`, `f16`) are well-chosen and orthogonal. Users can trim dependencies appropriately.

### Builder Pattern for Writer
`create(path).shape(dims).mode::<T>().finish()` is ergonomic and self-documenting.

---

## 🔴 Critical Design Issues

### 1. `Writer` Buffers the Entire File in RAM
```rust
let data = vec![0u8; data_size];  // ~8.5 GB for a 2048³ f32 volume
```
This directly contradicts the crate's "zero-allocation" claim. The `Writer` type is unsuitable for large files, and the API gives users no warning. The only alternative, `MmapWriter`, requires the `mmap` feature.

**Recommendation:** Either make `Writer` stream directly to the file (seek + write per block), or rename it to `BufferedWriter` and make `MmapWriter` (or a streaming writer) the primary write API.

### 2. `write_block_parallel` is Unix-Only
```rust
use std::os::unix::fs::FileExt;  // <-- This makes the `parallel` feature non-portable
self.file.write_all_at(encoded, offset)?;
```
This will fail to compile on Windows. A public API gated behind a feature flag should be cross-platform.

**Recommendation:** Use `std::fs::File::seek` + `write_all` with a mutex for cross-platform parallel writes, or document that `parallel` is Unix-only and cfg-gate it accordingly.

### 3. Path Arguments Use `&str` Instead of `AsRef<Path>`
```rust
pub fn open(path: &str) -> Result<Reader, Error>      // Should be AsRef<Path>
pub fn create(path: &str) -> WriterBuilder             // Should be AsRef<Path>
```
This prevents passing `PathBuf`, `&Path`, or `&OsStr` directly.

**Recommendation:** Change all path parameters to `P: AsRef<Path>`.

---

## 🟡 API Inconsistencies

### 4. `MmapWriter` Lacks the Builder Pattern
`Writer` is created via `WriterBuilder`, but `MmapWriter` uses a direct constructor:
```rust
let writer = create("out.mrc").shape([64,64,64]).mode::<f32>().finish()?; // Writer
let writer = MmapWriter::create("out.mrc", header)?;                        // MmapWriter
```
**Recommendation:** Provide `MmapWriterBuilder` or unify the creation APIs.

### 5. Arbitrary Block Reading is Hidden
`Reader::read_voxels` and `MmapReader::read_voxel_bytes` are `pub(crate)`. Users who want to read a single arbitrary block must instantiate a `blocks::<T>(shape)` iterator and call `.next()`.

**Recommendation:** Expose `read_block<T>(offset, shape)` and `read_block_converted<S, D>(offset, shape)` on both readers.

### 6. Extended Header is Inaccessible
`Reader::open` reads and immediately drops the extended header bytes:
```rust
let mut ext_data = vec![0u8; ext_size];
file.read_exact(&mut ext_data)?;  // ext_data is dropped
```
There is no accessor like `reader.ext_header()` or `reader.ext_header_bytes()`.

**Recommendation:** Store extended header bytes and expose `ext_header(&self) -> Option<ExtHeader>`.

### 7. No Immutable `SliceAccess`
The `SliceAccess` trait only provides `slice_mut`:
```rust
fn slice_mut<T: EndianCodec>(&mut self, z: usize) -> Result<&mut [T], Error>;
```
There is no `slice<T>(&self, z: usize) -> Result<&[T], Error>` for read-only access.

**Recommendation:** Add `slice()` to the trait, or provide it as a standalone method on `MmapReader`/`Writer`.

---

## 🟠 Ergonomics & Type Safety

### 8. `VoxelBlock::new` Panics
```rust
pub fn new(offset: [usize; 3], shape: [usize; 3], data: Vec<T>) {
    assert_eq!(data.len(), shape[0] * shape[1] * shape[2], "...");
}
```
Library APIs should not panic on invalid user input.

**Recommendation:** Add `VoxelBlock::try_new(...) -> Result<Self, Error>` and document that `new` panics on mismatch.

### 9. Header is Fully Public with No Validated Construction
All 56 fields of `Header` are `pub`, making it trivial to construct invalid state (e.g., `mode = 99`, `nlabl = 255`). The `Header::new()` constructor exists but users can bypass validation entirely.

**Recommendation:** Consider a `HeaderBuilder` with validated setters, or at minimum document which fields must be set for a valid file.

### 10. `Error::Io(String)` Loses Error Context
```rust
Io(String),  // "IO error: open file: ..."
```
This discards `std::io::ErrorKind` and source chains.

**Recommendation:** Use `Io(#[from] std::io::Error)` or at minimum `Io(String, #[source] std::io::Error)`.

### 11. Mode 0 Ambiguity is Not Handled by the Reader
Mode 0 (`Int8`) files are ambiguous — they may be signed or unsigned depending on the software that wrote them. The crate provides `M0Interpretation` and `reinterpret_m0`, but `Reader::slices::<i8>()` does not warn or guide users about this.

**Recommendation:** Consider a method like `reader.slices_mode0(interpretation: M0Interpretation)` or at least document the ambiguity prominently on `Reader::open`.

### 12. `CheckedConvert` is Orphaned
The `CheckedConvert` trait is defined and exported but **nowhere integrated** into the read/write API. All conversions in `write_converted` and `decode_and_convert` use the infallible `Convert` trait (which clamps silently).

**Recommendation:** Either integrate `CheckedConvert` into the API (e.g., `write_checked_converted`) or remove it from the public surface until it's wired up.

### 13. Iterator Type Proliferation
There are **10 public iterator types** (`SliceIter`, `MmapSliceIter`, `SliceIterConverted`, `MmapSliceIterConverted`, etc.). Most differ only in whether the backing storage is `Vec<u8>` or `Mmap`, and whether conversion is applied.

**Recommendation:** Consider unifying behind a generic `BlockIter<'a, R: BlockSource, T>` trait or type alias. This would cut the public API surface in half.

---

## 🟢 Minor Suggestions

| Issue | Suggestion |
|-------|-----------|
| `Mode` lacks `as_i32()` | Add `pub const fn as_i32(self) -> i32` instead of requiring `Mode::Float32 as i32` |
| `VolumeShape` lacks `Default` | Derive or implement `Default` for convenience |
| `Float32Complex::to_real` takes `ComplexToRealStrategy` | Consider `impl From<Float32Complex> for f32` with a default strategy, or a method per strategy |
| `Writer` uses `FileEndian::LittleEndian` hardcoded | Document that new files are always little-endian (this is crate policy, but should be explicit in `WriterBuilder` docs) |
| `Header::validate()` returns `bool` | Consider `validate_detailed() -> Result<(), HeaderError>` so users know *why* a header is invalid |

---

## Summary

The `mrc` crate has a **strong foundational design** — the iterator-centric API, type-level voxel safety, and conversion pipeline are all well-conceived. However, three API issues should be addressed before v1.0:

1. **The `Writer` RAM-buffering design** contradicts the crate's value proposition and forces users to `MmapWriter` for any non-trivial volume.
2. **Unix-only `parallel` feature** breaks cross-platform compatibility.
3. **`&str` paths** are unidiomatic for Rust file I/O.

Secondary priorities include unifying `MmapWriter` creation with the builder pattern, exposing block-level random access, and reducing the 10-type iterator zoo.
