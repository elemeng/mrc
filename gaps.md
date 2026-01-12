# MRC2014 Implementation Analysis: Gaps and Issues

This document analyzes the current implementation against the official MRC2014 specification and identifies gaps, issues, and recommendations.

## Analysis Summary

### ✅ Correctly Implemented Features

1. **Header Structure** (src/header.rs:1-1024)
   - All 56 4-byte words are correctly defined
   - 10x80-byte labels field (800 bytes) is correct
   - Total header size is exactly 1024 bytes
   - Field names match specification (nx, ny, nz, mode, etc.)

2. **Mode Support** (src/mode.rs)
   - All 8 MRC2014 modes are implemented: 0, 1, 2, 3, 4, 6, 12, 101
   - Byte sizes are correct for each mode
   - Complex types (modes 3, 4) correctly identified

3. **Extended Header** (src/header.rs:97-100)
   - EXTTYP field correctly positioned at bytes 8-11 of `extra`
   - NVERSION field correctly positioned at bytes 12-15
   - All standard EXTTYP codes recognized (CCP4, MRCO, SERI, AGAR, FEI1, FEI2, HDF5)

4. **Machine Stamp** (src/header.rs:213-216)
   - Default value `0x44 0x44 0x00 0x00` for little-endian is correct
   - Endian swapping includes machst field

---

## Critical Issues (Must Fix)

### 1. Endian Swapping Bug (src/header.rs:238-242)

**Current Implementation:**

```rust
// Machine stamp should also be swapped for proper cross-platform compatibility
// Swap the 4 bytes of the machine stamp
let machst = u32::from_le_bytes(self.machst);
self.machst = machst.swap_bytes().to_le_bytes();
```

**Issue:** This code is incorrect. It reads `machst` as little-endian, swaps bytes, then writes back as little-endian. This effectively reverses the byte order when it should just swap the 4 bytes as-is.

**Recommendation:** Change to:

```rust
self.machst.reverse(); // Simply reverse the 4 bytes
```

---

### 2. Memory Leaks (src/mrcfile.rs:95-100, 120-125)

**Current Implementation in `read_ext_header()`:**

```rust
let buffer_slice = Box::leak(buffer.into_boxed_slice());
Ok(buffer_slice)
```

**Current Implementation in `read_view()`:**

```rust
let buffer_slice = Box::leak(buffer.into_boxed_slice());
MrcView::new(self.header, buffer_slice)
```

**Issue:** Both methods leak memory by converting a Box to a leaked reference. The memory is never freed, causing memory leaks for each call.

**Recommendation:** Return owned data instead:

```rust
Ok(buffer.into_boxed_slice())
```

---

### 3. Missing MAP Field Validation (src/header.rs:209-212)

**Specification:** Word 53 (bytes 209-212) must contain "MAP " to identify file type.

**Current Implementation:**

```rust
pub map: [u8; 4],
```

**Issue:** No validation that this field contains "MAP ".

**Recommendation:** Add validation in `Header::validate()`:

```rust
pub fn validate(&self) -> bool {
    self.nx > 0
        && self.ny > 0
        && self.nz > 0
        && matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12 | 101)
        && self.map == *b"MAP "  // Add this check
}
```

---

## Important Issues (Should Fix)

### 4. Density Statistics Defaults (src/header.rs:77-79)

**Specification Note 5:** The spec states that certain values indicate statistics are not well-determined:

- `DMAX < DMIN` indicates not well-determined
- `DMEAN < (smaller of DMIN and DMAX)` indicates not well-determined
- `RMS < 0` indicates not well-determined

**Current Implementation:**

```rust
dmin: f32::NEG_INFINITY,
dmax: f32::INFINITY,
dmean: 0.0,
rms: 0.0,
```

**Issue:** The defaults don't follow the convention. According to the spec, `dmin` should be greater than `dmax` to indicate "not well-determined". Current defaults have `dmin < dmax`, which could be misinterpreted.

**Recommendation:** Change to:

```rust
dmin: f32::INFINITY,      // Set higher than dmax
dmax: f32::NEG_INFINITY,  // Set lower than dmin
dmean: f32::NEG_INFINITY, // Less than both
rms: -1.0,                // Negative indicates not well-determined
```

---

### 5. Missing NVERSION Default (src/header.rs:112)

**Specification Note 9:** NVERSION should be set to indicate MRC format version (20140 for original, 20141 for latest).

**Current Implementation:**

```rust
extra: [0; 100],  // This includes EXTTYP and NVERSION
```

**Issue:** NVERSION defaults to 0, which doesn't indicate any specific version.

**Recommendation:** Set default NVERSION to 20141 (latest version):

```rust
extra: [0; 100],
// Then in new():
self.set_nversion(20141);
```

---

### 6. Machine Stamp Not Used for Byte Order Detection (src/mrcfile.rs:68-75)

**Specification Note 11:** Machine stamp encodes byte ordering of data.

**Current Implementation:**

```rust
fn read_header(file: &File) -> Result<Header, Error> {
    let mut header_bytes = [0u8; 1024];
    file.read_exact_at(&mut header_bytes, 0)
        .map_err(|_| Error::Io)?;
    // ... reads header without checking machst
}
```

**Issue:** The implementation doesn't use the machine stamp to detect and automatically handle byte order. Users must manually call `swap_endian()`.

**Recommendation:** Add automatic byte order detection based on machst field.

---

### 7. MAPC/MAPR/MAPS Validation Missing (src/header.rs:68-70)

**Specification:** MAPC, MAPR, MAPS define axis correspondence (1,2,3 for X,Y,Z).

**Current Implementation:**

```rust
pub mapc: i32,  // 1-based index of column axis (1,2,3 for X,Y,Z)
pub mapr: i32,  // 1-based index of row axis (1,2,3 for X,Y,Z)
pub maps: i32,  // 1-based index of section axis (1,2,3 for X,Y,Z)
```

**Issue:** The fields are correctly defined, but there's no validation that they contain valid values (1, 2, or 3) and are all distinct.

**Recommendation:** Add validation:

```rust
pub fn validate(&self) -> bool {
    self.nx > 0
        && self.ny > 0
        && self.nz > 0
        && matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12 | 101)
        && self.map == *b"MAP "
        && matches!(self.mapc, 1 | 2 | 3)
        && matches!(self.mapr, 1 | 2 | 3)
        && matches!(self.maps, 1 | 2 | 3)
        && self.mapc != self.mapr
        && self.mapc != self.maps
        && self.mapr != self.maps
}
```

---

## Minor Issues (Nice to Have)

### 8. Packed4Bit Data Access (src/mode.rs:42)

**Specification Note 2:** Mode 101 is "4-bit data packed two per byte"

**Current Implementation:**

```rust
Self::Packed4Bit => 1, // 4 bits per value, 2 values per byte
```

**Issue:** The `byte_size()` returns 1, which is correct for storage, but there's no mechanism to:

- Extract individual 4-bit values
- Write individual 4-bit values
- Handle the packed/unpacked conversion

**Recommendation:** Add helper methods for packed 4-bit data access.

---

### 9. Extended Header Content Not Parsed (src/mrcfile.rs:95-100)

**Specification:** Extended header can contain various metadata types indicated by EXTTYP.

**Current Implementation:**

```rust
pub fn read_ext_header(&self) -> Result<&'static [u8], Error> {
    if self.ext_header_size == 0 {
        return Ok(&[]);
    }
    let mut buffer = alloc::vec![0u8; self.ext_header_size];
    self.file.read_exact_at(&mut buffer, 1024).map_err(|_| Error::Io)?;
    let buffer_slice = Box::leak(buffer.into_boxed_slice());
    Ok(buffer_slice)
}
```

**Issue:** The implementation reads extended header bytes but doesn't:

- Validate EXTTYP matches the actual content
- Parse specific extended header formats (SERI, FEI1, FEI2, etc.)
- Provide structured access to extended header metadata

**Recommendation:** Add parsers for common EXTTYP formats (SERI, FEI1, FEI2, HDF5).

---

### 10. Machine Stamp Parsing Missing (src/header.rs:213-216)

**Specification Note 11:** Bytes 213-214 contain 4 nibbles indicating representation of float, complex, integer, and character datatypes.

**Current Implementation:**

```rust
machst: [0x44, 0x44, 0x00, 0x00],
```

**Issue:** The implementation stores the machine stamp but doesn't provide methods to:

- Parse the nibble values (float/complex/integer/char representation)
- Validate the machine stamp
- Set appropriate values for different architectures

**Recommendation:** Add methods to parse and validate machine stamp nibbles.

---

### 11. Endian Swapping Incomplete (src/view.rs:165-170)

**Current Implementation in `swap_endian_bytes()`:**

```rust
Some(Mode::Float16) => {
    #[cfg(feature = "f16")]
    {
        let data = self.view_mut::<half::f16>()?;
        for val in data.iter_mut() {
            let bytes = bytemuck::bytes_of_mut(val);
            bytes.reverse();
        }
    }
    #[cfg(not(feature = "f16"))]
    {
        let data = self.view_mut::<u16>()?;
        for val in data.iter_mut() {
            *val = val.swap_bytes();
        }
    }
}
```

**Issue:** Mode 12 (Float16) is handled, but the implementation doesn't swap the header's machine stamp to reflect the new byte order after swapping.

**Recommendation:** Update machst after swapping to reflect the new byte order.

---

## Specification Limitations (Not Implementation Issues)

### 12. Handedness Not Well-Defined

**Specification Section: Handedness:** The spec notes that handedness is not well-defined and mentions FEI's EPU software places image origin in top-left corner.

**Current Implementation:** No support for indicating or handling handedness.

**Note:** This is a known limitation in the spec itself. Consider adding a comment documenting this limitation.

---

## Recommendations Priority

### High Priority

1. Fix endian swapping bug (src/header.rs:238-242)
2. Fix memory leaks (src/mrcfile.rs:95-100, 120-125)
3. Add MAP field validation (src/header.rs:209-212)
4. Fix density statistics defaults (src/header.rs:77-79)
5. Set NVERSION default to 20141 (src/header.rs:112)

### Medium Priority

6. Add automatic byte order detection based on machst (src/mrcfile.rs:68-75)
2. Add MAPC/MAPR/MAPS validation (src/header.rs:68-70)
3. Add extended header parsers for common formats (SERI, FEI1, FEI2, HDF5)

### Low Priority

9. Add Packed4Bit helper methods (src/mode.rs:42)
2. Add machine stamp parsing methods (src/header.rs:213-216)
3. Update machst after endian swapping (src/view.rs:165-170)
4. Document handedness limitation

---

## Conclusion

The implementation is largely correct and follows the MRC2014 specification well, with all header fields, modes, and basic functionality properly implemented. However, there are several issues that should be addressed:

**Most Critical:**

- Endian swapping bug in `Header::swap_endian()`
- Memory leaks from `Box::leak()`
- Missing MAP field validation

**Important:**

- Density statistics defaults don't follow spec convention
- NVERSION should default to 20141
- No automatic byte order detection
- No validation of MAPC/MAPR/MAPS values

The codebase is well-structured and the core functionality is solid, but these issues should be addressed for full compliance with the MRC2014 specification and to prevent memory leaks and incorrect behavior.
