Perfect timing to do this — you now have **enough architectural clarity** to safely squeeze performance *without* breaking correctness or the endian guarantees.

Below is a **comprehensive, staged refinement plan** that directly targets each observed bottleneck, while preserving:

* ✅ one-copy (file I/O) / zero-copy (mmap)
* ✅ lazy endian conversion
* ✅ semantic separation (Header / ExtHeader / DataBlock)
* ✅ safety (no `bytemuck`, no UB)

I’ll be very explicit about *what changes*, *why*, and *what it buys you*.

---

# Performance Refinement Plan

*(Encode / Decode hot path)*

---

## 0️⃣ Guiding invariants (non-negotiable)

Before touching code, freeze these rules:

1. **Raw bytes are canonical**
2. **No implicit allocation**
3. **Endian conversion is per-element and lazy**
4. **Mode dispatch happens once, never per voxel**
5. **All fast paths must be `#[inline]` and monomorphic**

Everything below follows from these.

---

## 1️⃣ Eliminate unnecessary intermediate allocations

### 🔴 Current issue

* `as_f32() -> Vec<f32>`
* iterator `.collect()`
* implicit buffer creation in helpers

### 🟢 Refinement

#### Introduce *two tiers* of APIs

##### Tier 1 — zero-copy / zero-alloc (core)

```rust
pub fn iter_f32(&self) -> impl Iterator<Item = f32> + '_
pub fn get_f32(&self, idx: usize) -> f32
pub fn read_f32_into(&self, out: &mut [f32])
```

✔ no allocation
✔ reusable buffers
✔ friendly to streaming and filters

##### Tier 2 — explicit allocation (convenience)

```rust
pub fn to_vec_f32(&self) -> Vec<f32>
```

🚨 rename `as_*` → `to_vec_*` to make cost explicit

---

## 2️⃣ Replace iterator patterns that block autovectorization

### 🔴 Current issue

Patterns like:

```rust
data.chunks_exact(4).map(|c| f32::decode(...))
```

This:

* hides stride
* hides alignment
* prevents LLVM vectorization

---

### 🟢 Refinement: indexed loops with explicit stride

```rust
#[inline]
fn decode_f32_slice(
    bytes: &[u8],
    endian: FileEndian,
    out: &mut [f32],
) {
    let n = out.len();
    let mut i = 0;
    let mut offset = 0;

    if endian.is_native() {
        // fast path (see §4)
    } else {
        while i < n {
            out[i] = f32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            i += 1;
            offset += 4;
        }
    }
}
```

✔ predictable stride
✔ no iterator state
✔ autovectorizes well

---

## 3️⃣ Remove per-element bounds checks & slice creation

### 🔴 Current issue

```rust
&bytes[i * 4..i * 4 + 4]
```

Costs:

* bounds check
* slice object
* pointer metadata

---

### 🟢 Refinement

* Validate length **once**
* Then index raw bytes manually

```rust
debug_assert!(bytes.len() >= n * 4);
let ptr = bytes.as_ptr();
```

Safe pattern (still no `unsafe`):

```rust
let b0 = bytes[offset];
let b1 = bytes[offset + 1];
...
```

✔ no slice creation
✔ single bounds check
✔ hot-loop friendly

---

## 4️⃣ Add native-endian fast paths (huge win)

### 🔴 Current issue

You always go through `FileEndian` logic.

---

### 🟢 Refinement

#### Step 1: detect once

```rust
let native = endian.is_native();
```

#### Step 2: branch once, outside loop

```rust
if native {
    // memcpy / transmute-like logic
} else {
    // byte swap path
}
```

---

### 🔥 Fast path implementation (safe)

For native-endian + alignment-safe types:

```rust
#[inline]
fn decode_f32_native(bytes: &[u8], out: &mut [f32]) {
    let len = out.len();
    for i in 0..len {
        let base = i * 4;
        out[i] = f32::from_le_bytes([
            bytes[base],
            bytes[base + 1],
            bytes[base + 2],
            bytes[base + 3],
        ]);
    }
}
```

LLVM will:

* recognize pattern
* vectorize
* possibly lower to `memcpy`

---

## 5️⃣ Kill mode-level dynamic dispatch in hot loops

### 🔴 Current issue

```rust
match self.mode {
    Mode::Float32 => { decode f32 }
    Mode::Int16 => { decode i16 }
    ...
}
```

inside `as_*()` or iteration

This causes:

* branch per call
* blocks inlining
* prevents specialization

---

### 🟢 Refinement: **dispatch once**

#### At API boundary

```rust
pub fn read_into(&self, out: &mut [f32]) -> Result<()> {
    match self.mode {
        Mode::Float32 => decode_f32(...),
        _ => Err(...)
    }
}
```

#### Hot loops are now monomorphic

```rust
decode_f32(bytes, endian, out);
```

✔ branch-free inner loops
✔ full inlining
✔ vectorization unlocked

---

## 6️⃣ Special-case endian-agnostic modes

### 🔴 Current issue

* `i8`
* `Packed4Bit`

Still go through endian logic.

---

### 🟢 Refinement

```rust
match mode {
    Mode::Int8 => fast_path_no_endian(),
    Mode::Packed4Bit => packed_decode(),
    _ => endian_aware_path(),
}
```

Especially for `i8`:

* no endian
* no stride
* no swap

This is effectively a memcpy + cast.

---

## 7️⃣ DataBlock / DataBlockMut structure change

### Before

```rust
DataBlock {
    bytes,
    mode,
    endian,
}
```

### After (performance-aware)

```rust
DataBlock {
    bytes,
    mode,
    endian,
    native_endian: bool, // cached
    voxel_size: usize,   // cached
}
```

✔ removes recomputation
✔ helps bounds validation
✔ cheaper inner loops

---

## 8️⃣ Encoding path symmetry (same rules)

Encoding suffers from the same issues:

* per-element branching
* slice creation
* mode checks

Apply **the exact same strategy**:

* dispatch once
* native fast path
* indexed loops
* no temp buffers

---

## 9️⃣ What NOT to do (important)

❌ No global byte swap
❌ No `bytemuck::cast_slice`
❌ No `unsafe` transmute of `[u8] → [T]`
❌ No hidden allocation in “cheap-looking” APIs

You’re building a *scientific* crate — correctness > micro-optimizations.

---

## 10️⃣ Expected performance outcome

After refactor:

| Case                  | Result                    |
| --------------------- | ------------------------- |
| mmap + native endian  | ~memcpy speed             |
| mmap + foreign endian | ~1.1–1.2× memcpy          |
| buffered I/O          | exactly one copy          |
| gaussian filter       | endianness cost invisible |
| streaming write       | O(k), no full rewrite     |

---

## Final takeaway

You’re now at the point where:

> **Performance is limited by memory bandwidth, not code structure.**

That’s the best possible outcome.
