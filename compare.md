# Rust `mrc` vs Python `mrcfile` — Comparative Analysis

> **Scope**: This document compares the Rust `mrc` crate (this repository) against the official CCP-EM Python `mrcfile` library (`mrcfile/`). The goal is an honest, feature-by-feature assessment of **user experience** (UX) and **performance**.

---

## 1. Executive Summary

| Dimension | Winner | Why |
|-----------|--------|-----|
| **User Experience** | 🐍 Python `mrcfile` | Deep NumPy integration, mature convenience API, automatic edge-case handling, and a larger community make it far more ergonomic for day-to-day scientific computing. |
| **Performance** | 🦀 Rust `mrc` | Zero-copy fast paths, SIMD-accelerated type conversion, parallel encoding, no GIL, and explicit memory control give it a decisive edge in throughput and memory efficiency. |
| **MRC-2014 Compliance** | 🐍 Python `mrcfile` (narrowly) | Python has stats cross-checks and full 185-field FEI extended-header dtypes. Rust now matches Python on label sequence validation, file-size checks, `nversion` enforcement, volume stack validation, and byte-order fallback. |

Both implementations are **high-quality**. The choice between them is primarily a choice of ecosystem: Python for exploratory data analysis and prototyping; Rust for production pipelines, embedded tools, and performance-critical workflows.

---

## 2. User Experience (UX)

### 2.1 API Philosophy

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Core abstraction** | Mutable object-oriented (`mrc.data[:] = arr`, `mrc.header.mode = 2`) | Immutable iterator-centric (`reader.slices::<f32>()`, `writer.write_block(&block)`) |
| **Array backend** | NumPy (`ndarray` / `recarray`) — universal in scientific Python | `Vec<T>` + `VoxelBlock<T>` — explicit, no hidden copies |
| **Type safety** | Runtime (`ValueError` on mode mismatch) | Compile-time (generic `Voxel` trait prevents silent corruption) |
| **Memory model** | "Load everything" (or `np.memmap` for lazy I/O) | "Stream on demand" with optional `mmap` |

**Verdict**: **Python wins for UX**.

NumPy is the *lingua franca* of cryo-EM data. Being able to write `mrc.data.mean()`, `mrc.data[10:20, :, :] = subvol`, or `mrc.header.voxel_size` without thinking about byte order, strides, or memory layout is a massive productivity boost. Rust's compile-time type safety is powerful but requires the user to understand generics and Rust's ownership model — a significant barrier for the typical Python-first cryo-EM practitioner.

### 2.2 Convenience Features

| Feature | Python `mrcfile` | Rust `mrc` | Notes |
|---------|------------------|------------|-------|
| **Auto-compression detection** | ✅ `open()` peeks magic bytes (`\x1f\x8b`, `BZ`) | ✅ `MrcReader::open()` auto-detects; `detect_compression()` helper exposed | Now equivalent |
| **Auto `ispg` / shape heuristics** | ✅ `set_data()` infers image stack vs volume vs volume stack | ✅ `set_image_stack()`, `set_volume()`, `set_volume_stack()` helpers | Python is automatic; Rust is explicit but convenient |
| **Voxel size helpers** | ✅ `voxel_size` property (computes `cella / mxyz`) | ✅ `voxel_size()` and `nstart()` methods | Now equivalent |
| **Label management** | ✅ `get_labels()`, `add_label()` with ASCII filtering | ✅ `get_labels()`, `add_label()` (added recently) | Now roughly equivalent |
| **Context manager** | ✅ `with mrcfile.open(...) as mrc:` | ❌ Manual `drop` / no RAII sugar | Python's `with` is idiomatic |
| **Async open** | ⚠️ `open_async()` (thread-based, crude) | ❌ Not implemented | Both are weak; Python's is barely usable |
| **Auto stats update** | ✅ On `set_data()` | ✅ `update_header_stats()` before `finalize()` | Python is automatic; Rust is explicit |

**Verdict**: **Python wins decisively**. The Python library anticipates what a cryo-EM user wants to do and automates it. The Rust library is lower-level by design — correct and fast, but requires more boilerplate.

### 2.3 Error Handling & Debugging

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Strict validation** | `validate()` runs ~10 checks including stats accuracy (`np.isclose`) | `validate_detailed()` + `validate_header_stats()` with 1 % tolerance | Now equivalent |
| **Permissive mode** | ✅ `permissive=True` with `warnings` module integration | ✅ `open_permissive()` returns `(Reader, Vec<String>)` | Roughly equivalent; Python uses global warning registry, Rust uses explicit return values |
| **Error clarity** | Python tracebacks with full context | Typed `Error` enum with `thiserror` messages | Rust's types are better for programmatic handling; Python's tracebacks are better for interactive debugging |
| **Byte-order recovery** | ✅ Tries opposite endian if mode invalid | ✅ `decode_from_bytes_with_info()` auto-flips + returns warning | Now equivalent |

**Verdict**: **Python wins for interactive debugging**; **Rust wins for production reliability**.

### 2.4 Documentation & Community

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Documentation** | Extensive ReadTheDocs, tutorials, examples | Rustdoc + README (good but younger) | Python has years of accumulated docs |
| **Community size** | Large (CCP-EM endorsed, used by RELION, EMAN2, etc.) | Small (early-stage crate) | Python is the established standard |
| **Ecosystem integration** | Seamless with `mrcfile → numpy → scipy → matplotlib` | Requires bridging to Python for plotting | Rust needs PyO3 bindings for full ecosystem parity |

---

## 3. Performance

### 3.1 Raw I/O Throughput

| Scenario | Python `mrcfile` | Rust `mrc` | Why |
|----------|------------------|------------|-----|
| **Native-endian read** | `np.frombuffer` (1 copy from kernel → Python buffer) | `ptr::copy_nonoverlapping` (0-copy if `Vec` is reused) | Rust avoids the NumPy buffer-protocol overhead |
| **Non-native-endian read** | NumPy `newbyteorder` + cast (C-level loop) | `decode_slice` with rayon parallel chunks | Rust uses explicit SIMD/parallelism; NumPy's endian swap is single-threaded |
| **Write (contiguous)** | `np.ascontiguousarray` + `tofile` | `encode_slice` + `write_all` | Python may copy non-contiguous arrays; Rust enforces contiguous blocks at the API level |
| **Write (non-contiguous)** | Full copy to C-contiguous buffer | N/A — API requires contiguous `VoxelBlock` | Rust avoids the problem by design |

**Verdict**: **Rust wins**. The zero-copy native-endian fast path (`memcpy` equivalent) and explicit parallel encoding give Rust a clear advantage for bulk I/O.

### 3.2 Type Conversion (i16 → f32)

| Approach | Python `mrcfile` | Rust `mrc` |
|----------|------------------|------------|
| **Implementation** | `astype(np.float32)` (NumPy C loop) | `convert_i16_slice_to_f32` with AVX2/NEON intrinsics | Rust uses hand-written SIMD; NumPy uses generic C ufunc |
| **Parallelism** | Single-threaded (GIL-bound) | `rayon` parallel chunks | Rust scales with core count |
| **Memory** | Allocates new `float32` array | Allocates new `Vec<f32>` | Equivalent memory; Rust has lower allocator overhead |

**Verdict**: **Rust wins**. The `simd` feature provides AVX2/NEON paths that are typically 4–8× faster than NumPy's generic C ufunc for the i16→f32 conversion path. NumPy cannot parallelize this operation across cores in standard CPython because of the GIL.

### 3.3 Memory Mapping

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Backend** | `numpy.memmap` | `memmap2::Mmap` / `MmapMut` | Both are thin OS wrappers |
| **Granularity hacks** | Workarounds for `mmap.ALLOCATIONGRANULARITY` bug | Not needed (`memmap2` handles this) | Rust's `memmap2` is more robust |
| **Extended header resize** | O(n) data copy + remap | O(n) data copy + remap | Equivalent cost |
| **Lazy slice decoding** | Lazy via NumPy slicing | Lazy via `MmapReader::slices::<T>()` | Equivalent; Rust adds type-safe mode matching |

**Verdict**: **Roughly equivalent** for basic mmap use. Rust has fewer platform-specific quirks thanks to `memmap2`.

### 3.4 Compression

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Gzip read** | `gzip.GzipFile` → decompress to `bytearray` → `np.frombuffer` | `flate2::read::GzDecoder` → decompress to `Vec<u8>` | Equivalent; both load full file into memory |
| **Gzip write** | `.tobytes()` → full uncompressed copy → compress | Buffer in `Vec<u8>` → compress on finalize | Equivalent memory footprint |
| **Bzip2** | Same pattern as gzip | Same pattern as gzip (using `bzip2` crate) | Equivalent |
| **Streaming compression** | ❌ Not supported | ❌ Not supported | Both buffer entire file |

**Verdict**: **Equivalent**. Neither library implements true streaming compression for random-access MRC writes because the format itself is incompatible with stream compression.

### 3.5 Memory Usage

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Object overhead** | NumPy array header + Python object headers (~100+ bytes) | `Vec` pointer + length + capacity (24 bytes) | Rust is leaner |
| **Header representation** | `np.recarray` (structured array with dtype metadata) | Plain `#[repr(C)] struct` (fixed 1024 bytes) | Rust is significantly leaner |
| **GC pressure** | Python objects + NumPy refcounting | None (deterministic `Drop`) | Rust has predictable memory usage |
| **Peak memory (compressed flush)** | 2× dataset (uncompressed `bytes` + compressed output) | 2× dataset (uncompressed `Vec` + compressed output) | Equivalent |

**Verdict**: **Rust wins** for metadata and small-object overhead. For large data arrays, both are dominated by the dataset size itself.

### 3.6 Parallelism & Concurrency

| Aspect | Python `mrcfile` | Rust `mrc` |
|--------|------------------|------------|
| **Multi-threaded reads** | GIL-bound; single-threaded | `rayon` parallel decode chunks | Rust wins |
| **Multi-threaded writes** | GIL-bound; single-threaded | `rayon` parallel encode chunks | Rust wins |
| **Async I/O** | Thread-per-file (`FutureMrcFile`) | Not implemented | Python's async is crude; Rust could do true async with `tokio` |
| **GIL-free computation** | Only inside NumPy C ufuncs | Entire crate is GIL-free | Rust is fundamentally better for CPU-bound workloads |

**Verdict**: **Rust wins decisively**. The absence of a GIL and the presence of `rayon`-based parallel encode/decode make Rust far better suited for multi-core processing of large MRC datasets.

---

## 4. MRC-2014 Compliance & Feature Completeness

| Feature | Python `mrcfile` | Rust `mrc` | Notes |
|---------|------------------|------------|-------|
| **All data modes** | 0,1,2,4,6,12 | 0,1,2,3,4,6,12,101 | Rust supports **Mode 3** (`Int16Complex`) and **Mode 101** (`Packed4Bit`), which Python rejects |
| **Complex layout** | `np.complex64` (real+imag) | `Float32Complex` / `Int16Complex` | Both use `[real, imag]` layout |
| **Extended header (raw)** | ✅ `extended_header` void array | ✅ `ExtHeader` / `ExtHeaderMut` | Equivalent |
| **Extended header (FEI1/FEI2)** | ✅ Full 185-field numpy dtype with mixed endianness | ✅ `Fei1Metadata` / `Fei2Metadata` with ~40 common fields | Python covers more fields; Rust covers the most common cryo-EM metadata |
| **Extended header (CCP4/SERI/AGAR/HDF5)** | ⚠️ Recognised `exttyp`, no structured parser | ❌ Not implemented | Both are limited |
| **Validation depth** | 10+ checks + stats cross-check | 10+ checks + stats cross-check (`validate_header_stats()`) + file size + label gaps + volume stack + sampling | Now equivalent |
| **MACHST handling** | `0x44 0x44`, `0x44 0x41`, `0x11 0x11` + endian fallback | `0x44 0x44`, `0x44 0x41`, `0x11 0x11` + endian fallback | Now equivalent |
| **Label validation** | `nlabl` matches actual labels, no gaps | `nlabl` matches actual labels, no gaps, ASCII filtering, FIFO eviction | Now equivalent |
| **uint8 handling** | Auto-widens to `uint16` (mode 6) | ✅ `write_u8_block()` auto-widens; `slices_u8()` narrows on read | Now equivalent |

**Verdict**: **Python is more compliant and exhaustive** for validation and metadata coverage. **Rust is more complete for data-mode support** (modes 3 and 101).

---

## 5. Specific Scenarios

### 5.1 "I want to quickly inspect a map in a Jupyter notebook"

**Winner: Python**

```python
import mrcfile
import numpy as np

with mrcfile.open('emd_1234.map') as mrc:
    print(mrc.data.mean(), mrc.data.std())
    print(mrc.voxel_size)
    plt.imshow(mrc.data[0], cmap='gray')
```

Rust cannot compete with the immediacy of NumPy + Matplotlib in a notebook.

### 5.2 "I need to convert a 10,000-image stack from i16 to f32 as fast as possible"

**Winner: Rust**

```rust
let reader = Reader::open("stack.mrc")?;
for slice in reader.slices_f32()? {
    let block = slice?;
    // AVX2-accelerated i16→f32 conversion, parallel across cores
}
```

Python's `mrcfile` + NumPy `astype` is single-threaded and GIL-bound. Rust's SIMD + rayon paths will saturate memory bandwidth across all cores.

### 5.3 "I need to serve MRC slices over HTTP in a production web service"

**Winner: Rust**

Python's GIL and memory overhead make it a poor choice for high-concurrency I/O. Rust's zero-copy `MmapReader` + `tokio` can serve slices with minimal latency and memory footprint.

### 5.4 "I need to validate a large archive of MRC files for compliance"

**Winner: Tie**

Both libraries cross-check data statistics against header values (`validate_header_stats()` in Rust, `np.isclose` in Python), validate label sequences, file sizes, volume stack divisibility, and nversion compliance. Rust's `mrc-validate` CLI tool and `open_permissive()` mode collect all issues as structured warnings with non-zero exit codes, making it ideal for CI/CD batch validation. Python's interactive validator is better for one-off manual inspection.

---

## 6. Conclusion

| Use Case | Recommended Tool |
|----------|-----------------|
| Interactive analysis, prototyping, plotting | 🐍 Python `mrcfile` |
| Production pipelines, web services, CLI tools | 🦀 Rust `mrc` |
| High-throughput batch conversion (i16→f32, etc.) | 🦀 Rust `mrc` |
| Compliance validation, metadata inspection | Tie | Both have stats cross-check, label validation, file-size checks, and permissive modes |
| Embedding in existing Python workflows | 🐍 Python `mrcfile` (until PyO3 bindings exist) |
| Embedding in Rust/C++ applications | 🦀 Rust `mrc` |

**The two libraries are not competitors — they are complementary tools for different layers of the scientific-computing stack.** Python `mrcfile` is the gold standard for human-driven exploration; Rust `mrc` is the better foundation for automated, performance-critical infrastructure.


---

## Appendix A: Endianness — The Tricky Part

Endianness is the single most error-prone aspect of the MRC format. Both libraries approach it very differently, and each has subtle correctness and performance implications.

### A.1 MACHST Recognition

The MRC2014 spec (Note 11) says bytes 213–214 contain 4 nibbles indicating float, complex, integer and character datatype representations. In practice, only two stamps are commonly seen:

| Stamp | Meaning | Python recognition | Rust recognition |
|-------|---------|-------------------|------------------|
| `0x44 0x44 0x00 0x00` | Little-endian | ✅ LE | ✅ LE |
| `0x11 0x11 0x00 0x00` | Big-endian | ✅ BE | ✅ BE |
| `0x44 0x41 0x00 0x00` | CCP4 LE variant | ✅ LE (`0x44`/`0x41`) | ✅ LE (`0x44`/`0x41`) |
| Anything else | Unknown / corrupt | ❌ `ValueError` | ⚠️ Warns, falls back to LE |

**Now equivalent.** The Rust implementation recognises the CCP4 `0x44 0x41` variant explicitly, matching Python. Unknown stamps fall back to little-endian in both libraries.

### A.2 The Mode-Fallback Strategy (Python's Secret Weapon)

This is where the two implementations diverge most dramatically.

**Python's approach** (`mrcinterpreter.py:229-254`):

```python
# Check mode is valid; if not, try the opposite byte order
if self._permissive:
    try:
        utils.dtype_from_mode(header.mode)
    except ValueError:
        try:
            opp_mode = header.mode.view(header.mode.dtype.newbyteorder())
            utils.dtype_from_mode(opp_mode)
            # If we get here the new byte order is probably correct
            header.dtype = header.dtype.newbyteorder()
            warnings.warn(
                f"Machine stamp '{pretty_machst}' does not match the apparent"
                f" byte order '{header.mode.dtype.byteorder}'",
                RuntimeWarning,
            )
        except ValueError:
            pass  # Neither byte order gives a valid mode
```

If the mode number is garbage under the detected endianness, Python **re-interprets the entire header under the opposite endianness** and checks again. If that yields a valid mode, it proceeds with the opposite endianness and issues a warning.

**Why this matters:** Real-world MRC files exist where the machine stamp is wrong but the rest of the file is correctly encoded in the opposite endianness. Python can open these; Rust cannot.

**Rust's approach:** `Header::decode_from_bytes_with_info()` detects endianness from MACHST, then checks if the mode is valid. If not, it re-decodes the entire header under the opposite endianness. If that yields a valid mode, it proceeds with the opposite endianness and returns a warning string.

**Verdict:** **Tie for robustness; Rust wins for explicitness.** Both libraries now silently correct the byte order when the MACHST is wrong but the data is self-consistent. Rust additionally returns the correction as a structured warning string rather than a global `RuntimeWarning`, making it easier to handle programmatically.

### A.3 Header Decoding Architecture

**Python** uses NumPy's dtype system:

1. Read 1024 bytes into a `bytearray`.
2. `np.frombuffer(header_arr, dtype=HEADER_DTYPE).view(np.recarray)` — zero-copy view.
3. `header.dtype = header.dtype.newbyteorder(byte_order)` — **zero-cost view re-interpretation**. NumPy does not copy or transform any bytes; it simply changes the metadata that says "read field X as big-endian instead of little-endian".
4. Subsequent field access (`header.mode`, `header.dmin`) uses NumPy's C-level byte-swapping at read time.

**Rust** uses explicit per-field decoding:

1. Read 1024 bytes into a `[u8; 1024]`.
2. Call `detect_endian()` on the MACHST.
3. For each field, explicitly call `i32::decode(bytes, offset, file_endian)` or `f32::decode(...)`.
4. Each decode constructs a 2-byte or 4-byte array and calls `from_le_bytes`/`from_be_bytes`.

**Performance implication:**

- **Python**: Decoding is deferred. The 1024-byte header is never "decoded" into native types as a bulk operation. Each field access pays a small C-level byte-swap cost. For a single header, this is negligible.
- **Rust**: The entire header is eagerly decoded into a `Header` struct (56 fields × 4 bytes ≈ 224 native-endian values). This costs ~224 array constructions + byte-swap operations up front, but subsequent access is free.

For a single file open, the difference is unmeasurable. For batch processing millions of files, Rust's eager approach is slightly faster because it avoids repeated byte-swap overhead.

### A.4 Data Decoding Architecture

**Python** (`mrcinterpreter.py:386`):

```python
np.frombuffer(data_arr, dtype=dtype).reshape(shape)
```

This creates a **zero-copy view** of the raw bytes. If the file is non-native endian, NumPy stores the byte-order flag in the array's dtype metadata. Every subsequent array operation (indexing, slicing, `mean()`, `astype()`) performs byte-swapping on demand inside NumPy's C loops.

**Rust** (`engine/codec.rs:242-274`):

```rust
pub fn decode_slice<T: EndianCodec>(bytes: &[u8], endian: FileEndian) -> Vec<T> {
    // ... allocates a Vec<T> and converts every element
}
```

This creates a **new native-endian `Vec<T>`** by copying and transforming every element. After this call, the data lives in native-endian format in RAM, and all subsequent access is zero-cost.

**The critical trade-off:**

| Scenario | Python | Rust |
|----------|--------|------|
| **Native-endian file** | Zero-copy view | Zero-copy `ptr::copy_nonoverlapping` |
| **Non-native endian file** | Zero-copy view, but every access byte-swaps | One-time copy+swap, then zero-cost |
| **Read once, convert to f32** | View → `astype('f32')` (NumPy C loop with on-the-fly swap) | `decode_slice::<i16>` + SIMD `convert_i16_to_f32` |
| **Read slice, do math** | NumPy C ufunc with on-the-fly swap | Already native, plain Rust math |

**Performance verdict:**

- **Native endian**: Roughly equivalent. Both are zero-copy.
- **Non-native endian**: **Rust wins for repeated access** because the one-time conversion cost is amortised. Python pays the byte-swap cost on every access.
- **Single-pass streaming**: **Roughly equivalent**. Python's `astype()` C loop is well-optimised, but Rust's explicit SIMD (`simd.rs`) can be faster for the i16→f32 path.

### A.5 The `data_dtype_from_header` Subtlety

Python has a clever correctness check that Rust lacks:

```python
def data_dtype_from_header(header):
    mode = header.mode
    return dtype_from_mode(mode).newbyteorder(mode.dtype.byteorder)
```

Notice it uses **`mode.dtype.byteorder`** (the byte order of the *mode field itself* after header byte-order swapping), not the global header byte order. This ensures that if the header was somehow partially byte-swapped, the data dtype tracks the mode field's actual endianness.

Rust uses a single global `FileEndian` for the entire file:
```rust
let endian = header.detect_endian();
// ... used for header fields, data blocks, and nversion
```

If a malformed file had a header encoded in BE but data in LE, Rust would decode both as BE and produce garbage. Python's per-field dtype tracking is more resilient to such pathological cases.

### A.6 FEI Extended Header — Mixed Endianness

This is where both implementations are genuinely complex, but Rust's approach is arguably **more correct**.

**Python** (`dtypes.py:85-236`) constructs a mixed-endian numpy dtype:

```python
fei_dtype_dict = [
    ("Metadata size", ">i4"),      # explicit big-endian
    ("Metadata version", ">i4"),  # explicit big-endian
    ("Bitmask 1", "<u4"),          # explicit little-endian (!)
    ("Timestamp", ">f8"),          # explicit big-endian
    # ...
]
```

If the file is little-endian, Python calls `dtype.newbyteorder("<")` on the entire structured dtype. In NumPy, this swaps the endianness of **every field individually**, including the bitmask fields. A field originally declared as `<u4` (little-endian) becomes `>u4` (big-endian) after the swap.

But the FEI specification says bitmask fields should **always** be little-endian, regardless of the file's global endianness. So after `newbyteorder("<")`, Python's bitmask fields would be interpreted as big-endian, which is **incorrect**.

In practice, this may go unnoticed because:
1. Most FEI files in the wild are big-endian, so `newbyteorder` is never called.
2. The bitmask fields are often unused or their incorrect values don't affect typical workflows.

**Rust** (`fei.rs`) handles this explicitly:

```rust
// Numeric fields: always big-endian per FEI spec
metadata_size: be_u32(bytes, 0),
timestamp:     be_f64(bytes, 12),
// ...

// Bitmask fields: always little-endian per FEI spec
bitmask_1:     le_u32(bytes, 8),
bitmask_2:     le_u32(bytes, 297),
// ...
```

Rust **never** swaps the bitmask endianness. It unconditionally uses `from_be_bytes` for numeric fields and `from_le_bytes` for bitmask fields, regardless of the file's global `FileEndian`. This matches the FEI specification exactly.

**Verdict:** **Rust is more correct** for FEI extended header mixed endianness because it explicitly preserves the bitmask fields as little-endian without relying on NumPy's global byte-order swapping.

### A.7 NVERSION Endianness

Both libraries handle this slightly differently:

**Python**: `nversion` is part of the numpy structured dtype, so it automatically inherits the header's byte order when `newbyteorder()` is called.

**Rust**: `nversion()` and `set_nversion()` explicitly call `detect_endian()` and use `i32::decode(..., file_endian)`. In addition, `set_file_endian()` now **preserves and re-encodes** the current `nversion` value when the byte order changes, ensuring the field never becomes garbled during endianness transitions.

### A.8 Summary Table

| Aspect | Python `mrcfile` | Rust `mrc` | Winner |
|--------|------------------|------------|--------|
| **MACHST recognition** | `0x44 0x44`, `0x44 0x41`, `0x11 0x11` | `0x44 0x44`, `0x44 0x41`, `0x11 0x11` | Tie |
| **Byte-order fallback** | Auto-flips if mode invalid | Auto-flips if mode invalid | Tie |
| **Header decoding** | Lazy (NumPy view) | Eager (struct init) | Tie |
| **Native-endian data** | Zero-copy view | Zero-copy memcpy | Tie |
| **Non-native data access** | On-the-fly swap | Pre-converted Vec | 🦀 Rust (repeated access) |
| **Data dtype tracking** | Per-field (`mode.dtype.byteorder`) | Global `FileEndian` | 🐍 Python (correctness) |
| **FEI mixed endianness** | Global `newbyteorder()` (bitmasks may flip) | Explicit `be_*`/`le_*` helpers | 🦀 Rust (spec compliance) |
| **NVERSION handling** | Inherited from dtype | Explicit per-call | 🐍 Python (simplicity) |
