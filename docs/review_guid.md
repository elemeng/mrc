# MRC Library Manual Review Guide: Data Flow & API Consistency

This guide will walk you through a systematic code review at five levels. As a beginner, focus on **tracing one data path at a time** (e.g., opening a file → reading a voxel) rather than understanding everything at once.

---

## 📋 **Level 0: Preparation (Do This First)**

### Set Up Your Review Environment

1. **Open these 5 files side-by-side**:
   - `lib.rs` (main types)
   - `header.rs` (Header struct)
   - `mode.rs` (Mode enum)
   - `view.rs` (MrcView/MrcViewMut)
   - `mrcfile.rs` (MrcFile/MrcMmap)

2. **Pick a test file**: Create a tiny 2×2×2 MRC file (32 bytes) to trace manually.

3. **Print key invariants** for each struct:

   ```rust
   // For every struct, ask: What must ALWAYS be true?
   // Example for DataBlock:
   // - bytes.len() must equal mode.byte_size() * voxel_count
   // - mode must match the actual data type in bytes
   ```

---

## 💾 **Level 1: Disk I/O Analysis (Where Data Enters/Leaves)**

### What to Look For

Every `read`/`write` call, file offset calculation, and buffer allocation from disk.

### Key Locations

**In `mrcfile.rs`:**

- `MrcFile::open()` line 30: **`file.read_exact_at(...)`** - reads ENTIRE file
- `MrcFile::create()` line 66: `file.write_all_at(&header_bytes, 0)`
- `MrcFile::write_view()` line 120: Three separate writes (header, ext, data)
- `MrcMmap::open()` line 196: `MmapOptions::new().map(&file)` - memory map

### Review Questions

1. **How many disk reads per operation?**
   - ❌ Bad: `MrcFile::open()` does **1 giant read** for whole file
   - ✅ Good: Should read **only header first**, then data on-demand

2. **Are offsets calculated correctly?**
   - Check: `data_offset = 1024 + nsymbt` (line 14 in header.rs)
   - Trace: For `nsymbt=100`, data starts at byte **1124**

3. **Does writing preserve atomicity?**
   - ❌ Bad: `write_view()` does **3 separate writes** - file is corrupt if interrupted
   - ✅ Good: Should write to temp file then atomically rename

### Manual Trace Exercise

```rust
// Simulate opening a 10×10×10 float32 file:
// - Header: 1024 bytes
// - Ext header: 0 bytes  
// - Data: 10×10×10×4 = 4000 bytes
// Total read: 5024 bytes loaded into RAM immediately
// Question: What if file is 5GB? 💥
```

---

## 🧠 **Level 2: Memory Allocation & Copying**

### What to Look For

Every `Vec::new()`, `to_vec_*()`, `copy_from_slice()`, and buffer creation.

### Key Locations

**In `lib.rs` (DataBlock methods):**

- `to_vec_f32()` line 231: `Vec::with_capacity` + `read_f32_into`
- `read_f32_into()` line 193: `bytemuck::cast_slice` (zero-copy) vs manual copy
- `iter_f32()` line 155: Creates an iterator that **copies each value**

**In `mrcfile.rs` (MrcFile):**

- Line 32: `buffer: alloc::vec![0u8; total_size]` - **single biggest allocation**
- Line 187: `data_cache: Option<DecodedData>` - cache stores **second copy** of data

### Review Questions

1. **Where does copying happen?**
   - ❌ **Double copy**: `MrcFile::open()` reads bytes → `data_f32()` decodes to new Vec → cache stores it
   - ✅ Should: Memory map once, decode in-place

2. **Are there unnecessary allocations?**
   - Look for `repeat_n(0.0, n).collect()` - initializes then overwrites
   - Better: `Vec::with_capacity(n)` + unsafe `set_len(n)`

3. **Does cache prevent re-allocation?**
   - ❌ Cache is **per MrcFile instance** - open 1000 files = 1000 caches
   - ❌ Cache stores **decoded data only** - can't share raw bytes

### Manual Trace Exercise

```rust
// Trace: view_f32() → data_f32() → decode_into()
// Path:
// 1. data_f32() checks cache (miss) → allocates Vec<f32>
// 2. decode_into() calls to_vec_f32() → allocates AGAIN? No, same vec
// 3. Cache stores DecodedData::F32(vec)
// 4. Next call returns reference to cached vec
// 
// Problem: What if you call data_i16() then data_f32()? Cache is overwritten!
```

---

## ⚙️ **Level 3: CPU & Branch Analysis (Hot Loops)**

### What to Look For

Loops, `if` statements inside loops, and repeated calculations.

### Key Locations

**In `lib.rs` (DataBlock iterators):**

- `iter_f32()` line 155-179: `if file_endian.is_native()` **checked per voxel**
- `read_f32_into()` line 193-204: Same branch, but outside loop (✅ good)
- `packed4bit_iter()` line 527: **Bug**: yields 1 value/byte instead of 2

**In `header.rs` (validation):**

- `validate()` line 349-364: Called **every time** view is created

### Review Questions

1. **Are branches inside loops?**

   ```rust
   // ❌ BAD (inside loop):
   for i in 0..n {
       if file_endian.is_native() { // Branch repeated n times!
           // ...
       }
   }
   
   // ✅ GOOD (outside loop):
   if file_endian.is_native() {
       for i in 0..n { /* ... */ }
   } else {
       for i in 0..n { /* ... */ }
   }
   ```

2. **Are calculations repeated?**
   - `len_voxels()` recalculates `nx*ny*nz` every call - store it in header

3. **Is there a 'fast path'?**
   - ✅ `bytemuck::cast_slice` for native endian is zero-copy
   - ❌ Non-native path does **byte-by-byte copy** - could use SIMD

### Manual Trace Exercise

```rust
// Count operations for get_f32(42) in big-endian file:
// 1. assert!(mode == Float32) - 1 check
// 2. file_endian.is_native() - 1 branch
// 3. byte array extraction - 4 reads
// 4. f32::from_be_bytes - 1 conversion
// Total: ~7 operations per voxel
//
// Now multiply by 1 million voxels = 7 million branches
// Branch prediction fails → pipeline stalls → slow
```

---

## 🗄️ **Level 4: Data Model Consistency**

### What to Look For

Struct field relationships, enum variants, and invariants that must hold.

### Key Locations

**In `lib.rs`:**

- `DataState` enum line 98: **Raw vs Decoded** - can both exist? Should be mutually exclusive
- `FileEndian` vs `OutputEndian` - two ways to specify endianness

**In `view.rs`:**

- `MrcView` line 8: Holds `Header` by value (copied) + references to data
- `MrcViewMut` line 59: Same but with mutable data

**In `header.rs`:**

- `Header` line 12: **#[repr(C, align(4))]** - required for memory mapping, but no safety checks

### Review Questions

1. **What happens if invariants are broken?**
   - User modifies `header.mode = 999` → `view.mode()` returns `None` → `data_f32()` fails

2. **Are there duplicate sources of truth?**
   - `Header.nx, ny, nz` define dimensions
   - `DataBlock.bytes.len()` also implies dimensions
   - **Who wins if they disagree?** → `MrcView::from_parts` validates, but `DataBlock` doesn't

3. **Is the API symmetric?**
   - ✅ You can `decode_f32()` from `DataBlock`
   - ❌ You **cannot** encode back to `DataBlock` (no `encode_f32()` method)

### Manual Trace Exercise

```rust
// Create a MrcView with inconsistent data:
let header = Header { nx: 10, ny: 10, nz: 10, mode: 2, ... };
let data = [0u8; 100]; // Only 100 bytes, needs 4000

let view = MrcView::from_parts(header, &[], &data);
// Result: Err(Error::InvalidDimensions)
// Good! Validation caught it.

// But what about:
let view = MrcView::from_parts(header, &[], &data).unwrap();
let block = view.data;
assert_eq!(block.len_voxels(), 10*10*10); // Returns 25 (100/4)! Wrong!
// DataBlock trusts the mode, not the header dimensions
```

---

## 🌊 **Level 5: Code Flow Tracing (Follow One Path)**

### Pick ONE user operation and trace it completely

### Path A: Open → Read F32 Voxel

```rust
// User code:
let mut file = MrcFile::open("test.mrc")?;
let value = file.data_f32()?[42];
```

**Trace steps:**

1. `MrcFile::open()`:
   - ✅ Reads 1024-byte header
   - ✅ Validates header
   - ❌ **Allocates `buffer` for WHOLE file** (5GB? 💥)
   - ✅ Reads ext header + data into buffer

2. `data_f32()`:
   - ✅ Checks cache (cold miss first time)
   - ✅ Gets mode from header
   - ✅ Gets endianness
   - ❌ Creates `DataBlock` referencing `buffer[self.ext_header_size..]`
   - ❌ **Allocates new Vec<f32>** via `decode_into`
   - ✅ Decodes with `read_f32_into`
   - ❌ **Stores decoded vec in cache** (now have 2 copies in RAM)

3. Indexing `[42]`:
   - ✅ Returns reference to cached data

**Bottleneck**: 2× memory usage (raw + decoded). For 5GB file, need **10GB RAM**.

### Path B: Create → Write Data

```rust
// User code:
let mut file = MrcFile::create("out.mrc", header)?;
file.write_data(&raw_bytes)?;
```

**Trace steps:**

1. `MrcFile::create()`:
   - ✅ Writes header
   - ❌ Allocates `buffer` sized for whole file
   - ❌ **Writes zeros to disk** for ext+data (slow for large files)

2. `write_data()`:
   - ✅ Writes to disk via `write_all_at`
   - ✅ Copies to `self.buffer`
   - ❌ **Does NOT update header stats** (dmin, dmax, dmean remain wrong)

**Inconsistency**: File on disk has correct data but stale header stats.

---

## ✅ **Your Review Checklist**

Print this and check each box as you verify:

### Disk I/O

- [ ] Can I read only the header without loading data?
- [ ] Are file offsets calculated once and reused?
- [ ] Are writes atomic (all-or-nothing)?

### Memory

- [ ] Count allocations: How many `Vec::new()` per operation?
- [ ] Are there `clone()` calls on large data?
- [ ] Does cache store duplicates of same data?
- [ ] Are buffers pre-sized correctly (no realloc)?

### CPU

- [ ] Find every `for` loop - is there an `if` inside?
- [ ] Are endianness checks outside loops?
- [ ] Are calculations cached (e.g., `nx*ny*nz`)?
- [ ] Is there a slow path that's taken often?

### Data Model

- [ ] Pick a struct - list all invariants that MUST be true
- [ ] Find all `pub` fields - can they break invariants?
- [ ] Are enums non-exhaustive (`#[non_exhaustive]`) where needed?
- [ ] Is API symmetric (if you can decode, can you encode)?

### Code Flow

- [ ] Trace one path on paper: User call → every function → return
- [ ] Mark every allocation with 📦 emoji
- [ ] Mark every copy with 🔄 emoji
- [ ] At the end, count 📦 and 🔄 - should be minimal

---

## 🎯 **Beginner-Focused Fixes to Start With**

### 1. **Remove the Double Copy in `MrcFile`**

```rust
// BEFORE (line 32 in mrcfile.rs):
let mut buffer = alloc::vec![0u8; total_size];

// AFTER:
let mut buffer = alloc::vec::Vec::with_capacity(total_size);
unsafe { buffer.set_len(total_size); } // Skip zero-initialization
```

### 2. **Fix the Packed4Bit Iterator Bug**

```rust
// BEFORE (line 527 in lib.rs):
core::iter::once(value)

// AFTER:
core::iter::once(value.first()).chain(core::iter::once(value.second()))
```

### 3. **Make Cache Key-Based**

```rust
// BEFORE: One cache slot
data_cache: Option<DecodedData>

// AFTER: Cache by type
data_cache: HashMap<DecodeTarget, Arc<dyn Any>>
```

### 4. **Add Validation on Header Mut**

```rust
// BEFORE (header.rs line 282):
pub fn header_mut(&mut self) -> &mut Header { &mut self.header }

// AFTER:
pub fn set_mode(&mut self, mode: Mode) -> Result<(), Error> {
    if !mode.is_supported() { return Err(...); }
    self.header.mode = mode as i32;
    Ok(())
}
```

---

## 📚 **How to Practice**

1. **Start Small**: Pick ONE function like `read_f32_into()` and draw its memory layout on paper
2. **Use `dbg!()`**: Insert temporary prints to see actual sizes:

   ```rust
   let vec = self.to_vec_f32()?;
   dbg!(vec.len(), vec.capacity());
   ```

3. **Write a Test**: Create a test that opens a 2GB file (use `fallocate -l 2G dummy.mrc`) - does it crash?
4. **Count Lines**: For each API, count lines of code a user needs to write:
   - `MrcFile`: ~5 lines
   - `MrcView`: ~8 lines (manual decode)
   - Goal: Should be equal!

---

This systematic approach will help you find the exact spots where data is copied twice or APIs diverge. Focus on **one path at a time** and keep notes on each inconsistency you find.
