# Code Review: mrc Rust Crate

## Executive Summary

This is a well-structured Rust library for MRC-2014 file format handling with a clean architecture, good separation of concerns, and solid no_std support. However, there are several areas for improvement including API inconsistencies, incomplete error handling, potential logical issues, and missing safety checks.

---

## 1. Code Inconsistencies

### 1.1 Mismatched API Patterns

**Issue**: Inconsistent method naming between `RawHeader` and `Header`:
- `RawHeader::exttyp()` returns `[u8; 4]` but `Header::exttyp` is a public field
- `RawHeader::set_exttyp()` takes `[u8; 4]` but `Header::set_exttyp_str()` takes `&str`
- `RawHeader::nversion()` takes `FileEndian` parameter but `Header::nversion` is just `i32`

**Suggestion**: Align APIs - either make both use methods or both use public fields consistently.

### 1.2 Feature Gate Inconsistencies

**Issue**: In `src/lib.rs` lines 71-96, `std` feature is repeatedly checked:
```rust
#[cfg(feature = "std")]
pub mod io;

#[cfg(feature = "std")]
pub mod storage;
// ... etc
```

Could be grouped under a single `cfg` block for cleaner code.

### 1.3 Mode Validation Inconsistency

**Issue**: `Mode::is_supported()` checks for f16 feature at runtime, but `Header::try_from` also checks this at conversion time. This is redundant double-checking.

**Location**: `src/mode.rs:94-99` and `src/header/validated.rs:266-270`

---

## 2. Incomplete Implementations

### 2.1 Missing Packed4Bit Support

**Issue**: `Mode::Packed4Bit` (mode 101) is defined but not actually implemented:
- `VolumeData::from_bytes()` returns `Error::InvalidMode` for Packed4Bit
- No encoding implementation for 4-bit packed data
- No voxel type for 4-bit data

**Location**: `src/dynamic.rs:66-69`

**Suggestion**: Either implement full support or remove the mode until it's ready.

### 2.2 Incomplete Error Information

**Issue**: `Error::Io` only contains a String message, losing the original `std::io::Error`:
```rust
Io(alloc::string::String),  // Should be Io(std::io::Error)
```

This prevents users from matching on specific IO error kinds.

**Location**: `src/error.rs:40`

### 2.3 Unused Storage Trait

**Issue**: The `Storage` trait in `src/storage.rs` is defined but never actually used by `Volume`. `Volume` uses `AsRef<[u8]>` directly instead of the `Storage` trait.

**Suggestion**: Either integrate `Storage` into `Volume` or remove the trait if it's not needed.

### 2.4 Missing Validation on Header Write

**Issue**: `Header::try_from<RawHeader>` validates dimensions, mode, and axis map, but `Header -> RawHeader` conversion (`From<Header> for RawHeader`) doesn't validate before writing.

**Location**: `src/header/validated.rs:326-384`

---

## 3. Logical Errors

### 3.1 Endianness Conversion Bug

**Issue**: The endianness conversion logic is confusing and potentially incorrect:
```rust
let decode_i32 = |v: i32| -> i32 {
    match file_endian {
        FileEndian::Little => i32::from_le(v.to_le()),
        FileEndian::Big => i32::from_be(v.to_be()),
    }
};
```

This code converts to LE/BE and then interprets as LE/BE. The `to_le()` and `to_be()` methods convert FROM native endianness. So for a little-endian file on a little-endian machine:
- `v.to_le()` is a no-op (already little-endian)
- `i32::from_le()` is also a no-op

This works by accident on little-endian machines but the logic is confusing. Should use clearer:
```rust
match file_endian {
    FileEndian::Little => i32::from_le_bytes(v.to_ne_bytes()),
    FileEndian::Big => i32::from_be_bytes(v.to_ne_bytes()),
}
```

**Location**: `src/header/validated.rs:236-250`

### 3.2 Data Size Calculation Inconsistency

**Issue**: Two different calculations for data size:
- `RawHeader::data_size()` uses `saturating_mul` and handles mode 101 specially
- `Header::data_size()` uses simple multiplication and handles mode 101 with `div_ceil(2)`

These could diverge in edge cases. The `RawHeader` version is more defensive.

**Location**: `src/header/raw.rs:170-192` vs `src/header/validated.rs:131-140`

### 3.3 Axis Map Strides Calculation Assumes Standard Ordering

**Issue**: `AxisMap::strides()` takes `shape: [usize; 3]` but the stride calculation assumes X is the fastest varying dimension (standard ordering). For non-standard axis mappings, the strides would be incorrect.

```rust
let nx = shape[0];  // Assumes shape[0] is X dimension
let ny = shape[1];  // Assumes shape[1] is Y dimension
```

But if `column=3` (Z is fastest), then `nx` should be `shape[2]`.

**Location**: `src/axis.rs:100-128`

### 3.4 Volume Strides Ignore Axis Map

**Issue**: `Volume::new()` calculates strides as `[1, shape[0], shape[0] * shape[1]]` which assumes standard XYZ ordering, ignoring the header's `axis_map` field entirely.

**Location**: `src/volume.rs:78`

### 3.5 Extended Header Length Not Validated

**Issue**: When reading, `nsymbt` (extended header size) is used without validation. A malicious file could set `nsymbt` to a huge value causing OOM.

**Location**: `src/io/reader.rs:45-49`

---

## 4. Redundancy

### 4.1 Duplicate Mode-to-Byte-Size Logic

**Issue**: Mode byte size is defined in both:
- `Mode::byte_size()` in `src/mode.rs:58-68`
- `RawHeader::data_size()` inline match in `src/header/raw.rs:179-185`
- `Encoding::SIZE` via trait in `src/encoding.rs`

**Suggestion**: Use `Encoding::SIZE` consistently.

### 4.2 Duplicate Data Size Calculations

**Issue**: Data size calculation appears in:
- `RawHeader::data_size()`
- `Header::data_size()`
- `Header::file_size()`

### 4.3 Unnecessary PhantomData

**Issue**: `Volume` stores `PhantomData<T>` but `T` is already constrained by `Encoding` trait which has `MODE` constant. The type could potentially be simplified.

### 4.4 ComplexI16/ComplexF32 Implement Sealed Twice

**Issue**: Complex types implement `Sealed` in both `voxel.rs` and via the pattern - this is minor but the sealed trait pattern could be cleaner.

---

## 5. Performance Issues

### 5.1 Unnecessary Allocation in Reader

**Issue**: `MrcReader::read_data()` always allocates a new Vec, even for operations that could use a pre-allocated buffer.

**Location**: `src/io/reader.rs:89-96`

### 5.2 Volume Iteration Decodes Per Element

**Issue**: `Volume::iter()` calls `T::decode()` for every element, which for complex types involves multiple branches on endianness:
```rust
(0..len).map(move |i| {
    let offset = i * T::SIZE;
    T::decode(endian, &bytes[offset..offset + T::SIZE])
})
```

For native-endian files, this could be optimized to use `bytemuck::cast_slice` directly.

**Location**: `src/volume.rs:162-170`

### 5.3 Dynamic Dispatch Always Allocates

**Issue**: `VolumeData::from_bytes` always uses `Vec<u8>` backing, no support for borrowed data or mmap.

### 5.4 No Streaming Support

**Issue**: The library loads entire files into memory. For large cryo-EM volumes (can be GBs), there's no chunked/streaming API for reading partial data.

---

## 6. Safety and Correctness Issues

### 6.1 Unsafe Without Documentation

**Issue**: `RawHeader::zeroed()` uses `unsafe` but the safety comment is minimal:
```rust
// SAFETY: RawHeader is Zeroable
unsafe { core::mem::zeroed() }
```

**Location**: `src/header/raw.rs:159-162`

### 6.2 MmapStorage Uses Unchecked Cast

**Issue**: `MmapStorage::as_slice()` uses `bytemuck::cast_slice` without verifying the data content is valid for the type. For files with invalid bit patterns (e.g., NaN, invalid float representations), this is UB.

**Location**: `src/storage.rs:144`

### 6.3 Missing Alignment Check on File Read

**Issue**: When reading files, data alignment is not verified before creating views. This could lead to unaligned access on some architectures.

### 6.4 PartialEq on Floats

**Issue**: `RawHeader` derives `PartialEq` which includes f32 fields. This is technically fine but can be surprising with NaN values.

---

## 7. API Design Issues

### 7.1 Header Setters Don't Return Self

**Issue**: Header setters like `set_dimensions()`, `set_cell_dimensions()` don't return `&mut Self`, preventing method chaining:
```rust
// Can't do this:
header.set_dimensions(64, 64, 64)
      .set_cell_dimensions(100.0, 100.0, 100.0);
```

### 7.2 VolumeData Missing Methods

**Issue**: `VolumeData` has `as_f32()`, `as_i16()`, `as_u16()` but missing:
- `as_i8()`
- `as_complex_i16()`
- `as_complex_f32()`
- `as_f16()` (with feature)

### 7.3 No Builder Pattern for Header

**Issue**: Creating a valid header requires multiple field assignments. A builder would be more ergonomic:
```rust
let header = Header::builder()
    .dimensions(512, 512, 256)
    .mode(Mode::Float32)
    .voxel_size(1.0, 1.0, 1.0)
    .build()?;
```

---

## 8. Documentation Issues

### 8.1 Doc Examples Use `ignore`

**Issue**: `src/lib.rs` documentation examples use `ignore` instead of `no_run` or proper testable examples:
```rust
//! ```ignore
//! use mrc::{MrcReader, Mode};
//! ```

### 8.2 Missing Panic Documentation

**Issue**: Methods like `Volume::get()`, `Volume::get_at()`, `Volume::set()` can panic but don't document when.

---

## Specific Recommendations (Priority Order)

### High Priority

1. **Fix endianness conversion logic** - Clarify and verify the byte conversion in header validation
2. **Fix axis map stride calculation** - Currently ignores non-standard axis mappings
3. **Add OOM protection** - Validate `nsymbt` before allocating
4. **Either implement or remove Packed4Bit** - Currently a stub

### Medium Priority

5. **Unify data size calculations** - Single source of truth
6. **Add builder pattern for Header** - Better ergonomics
7. **Complete VolumeData API** - Add missing type accessors
8. **Add alignment verification** - For mmap and file reading

### Low Priority

9. **Unify RawHeader/Header APIs** - Consistent naming
10. **Optimize Volume::iter()** - Special-case native endian
11. **Use Io(std::io::Error)** - Preserve error information
12. **Add streaming/chunked API** - For large files

---

## Overall Assessment

| Aspect | Rating | Notes |
|--------|--------|-------|
| Architecture | Good | Clean separation of concerns |
| API Design | Good | Generally ergonomic, some inconsistencies |
| Correctness | Fair | Potential endianness and axis map bugs |
| Performance | Good | Zero-copy where possible, some optimization opportunities |
| Safety | Good | Minimal unsafe, well-contained |
| Documentation | Good | Well-documented, some gaps |
| Testing | Fair | Basic coverage, needs more edge cases |

The crate is well-architected and suitable for production use after addressing the high-priority correctness issues.
