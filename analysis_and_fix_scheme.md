## 1. High-level data flow (what actually happens)

### File → Memory pipeline

**`mrcfile.rs`**

```
File / mmap
  ├─ read header (1024 bytes)
  │    └─ decode into Header (native endian)
  ├─ read ext header (opaque Vec<u8>)
  └─ read data section (Vec<u8> or mmap slice)
```

**Key decision point**

* Endianness is resolved **once** from header
* Raw voxel bytes are kept **undecoded** until a view asks for them

---

### Header flow

**`header.rs`**

```
[raw bytes]
   ↓
decode_header()
   ↓
Header {
    nx, ny, nz,
    mode,
    stats,
    labels,
    endian
}
```

**Observations**

* Header is eagerly decoded
* Endianness becomes a *global implicit dependency* for later decoding
* Header dimensions drive *every* later allocation

---

### Mode-driven decoding

**`mode.rs`**

```
Mode enum
   ├─ byte size per voxel
   ├─ decode functions
   └─ type-level intent (i8, f32, complex, packed4bit…)
```

**Actual flow**

```
Vec<u8> (raw)
   ↓
as_f32() / as_i16() / as_complex() ...
   ↓
Vec<T>
```

Each `as_*`:

* Validates mode
* Allocates a fresh Vec
* Iterates and decodes element-by-element

---

### View layer

**`view.rs`**

```
MrcFile
   └─ View<'a>
        ├─ shape (nx, ny, nz)
        ├─ stride
        └─ reference to raw bytes
```

**Important**

* Views are *logical only*
* No shared decoded cache
* Multiple views → repeated decoding

---

## 2. Inconsistencies in data flow (real issues)

### ❌ 1. Header is native-endian, data is file-endian — but API hides this

You state this clearly in docs, but **the API makes it easy to forget**:

```rust
self.header.nx // native
self.bytes     // file endian
```

**Problem**

* `Mode::decode` silently depends on `file_endian`
* Nothing in the type system enforces “decoded vs raw”

**Effect**

* Easy to accidentally mix decoded header values with undecoded data
* Hard to reason about correctness in downstream code

**Fix**
Introduce explicit states:

```rust
struct RawData<'a> { bytes: &'a [u8], endian: Endian }
struct DecodedData<T> { values: Vec<T> }
```

This makes invalid states unrepresentable.

---

### ❌ 2. Dimensions validated too late

Right now:

* Header dimensions are read
* Allocation happens later in decode paths

**Problem**

* You can create views and even mmap with invalid dimensions
* Errors surface deep in decode paths

**Fix**
Validate *once* in `MrcFile::open`:

```rust
nx * ny * nz * mode.bytes() == data_len
```

This also avoids repeated length checks.

---

### ❌ 3. `Mode` checks duplicated everywhere

Each `as_*` does:

```rust
if self.mode != Mode::F32 {
    return Err(Error::InvalidMode);
}
```

**Problem**

* Mode correctness is a *runtime invariant*
* But decoding functions assume it anyway

**Fix**
Centralize:

```rust
fn decoder(&self) -> Decoder<'_>
```

Or expose typed access:

```rust
fn data<T: MrcType>(&self) -> Result<DataView<T>>
```

---

## 3. Performance impairments (important)

### 🐌 1. Repeated full decode (biggest issue)

Every call to:

* `as_f32`
* `as_i16`
* `as_complex`
* etc.

→ allocates a **new Vec** and decodes everything again.

**Impact**

* O(N) time *per call*
* Massive overhead for large cryo-EM volumes
* Especially bad in analysis pipelines

**Fix (high value)**
Add **lazy decode + caching**:

```rust
enum DataCache {
    Raw(Vec<u8>),
    F32(Vec<f32>),
    I16(Vec<i16>),
}
```

Decode once, reuse forever.

---

### 🐌 2. Byte-by-byte decoding instead of chunked reads

Example pattern:

```rust
for chunk in self.bytes.chunks_exact(4) {
    Float32::decode(self.file_endian, chunk)
}
```

**Problem**

* Prevents vectorization
* Poor cache behavior

**Fix**
When endian matches native:

```rust
bytemuck::cast_slice::<u8, f32>()
```

When endian differs:

* Decode in blocks
* Use `read_unaligned + swap_bytes` on slices

This alone can be **5–10× faster**.

---

### 🐌 3. Packed4Bit expansion doubles memory eagerly

```rust
Vec::with_capacity(self.bytes.len() * 2)
```

**Problem**

* Immediate expansion
* Often only partially used downstream

**Fix**
Provide iterator-based decoding:

```rust
fn packed4bit_iter(&self) -> impl Iterator<Item = Packed4Bit>
```

Zero allocation until needed.

---

### 🐌 4. View layer doesn’t amortize cost

Views are cheap structurally, but:

* Each view decode → full decode again

**Fix**
Make views reference **decoded cache**, not raw bytes.

---

## 4. Design-level inconsistency

### ❌ “Safe, efficient” vs actual behavior

Your crate *claims*:

> “Safe, efficient MRC I/O”

Reality:

* Safety ✅
* Efficiency ❌ for repeated access

This is not a Rust problem — it’s a **data ownership problem**.

---

## 5. Concrete improvement roadmap (in order)

1. **Validate dimensions once at open**
2. **Separate raw vs decoded types**
3. **Add decode caching**
4. **Use bytemuck for native-endian fast paths**
5. **Switch packed formats to iterators**
6. **Make views borrow decoded data**

---

## 6. Summary (tl;dr)

**What’s inconsistent**

* Endian state is implicit
* Mode correctness is runtime-only
* Dimensions validated late

**What hurts performance**

* Full re-decode on every access
* No decode caching
* Byte-wise decoding
* Eager expansion of packed formats

**Big win**

> Cache decoded data + use zero-copy casts when endian matches.

Perfect, this is a good moment to formalize it 👍
I’ll give you **two artifacts**:

1. **A concrete fix plan** (engineering-oriented, stepwise, low ambiguity)
2. **An “LLM vibe coding doc”** — the kind of internal design note you (or an LLM) can follow while refactoring without getting lost

I’ll keep it Rust-native and aligned with your crate’s intent (scientific I/O, not toy abstractions).

---

# 1. Fix Plan (Actionable, Ordered)

This is ordered by **impact / effort ratio**.

---

## Phase 0 — Guardrails (small, high leverage)

### 0.1 Validate invariants at file open

**Goal**: fail early, simplify later code.

Add to `MrcFile::open`:

* Validate:

  * `nx * ny * nz * mode.bytes_per_voxel() == data_len`
  * supported `mode`
  * sane dimensions (non-zero, no overflow)

**Effect**

* Removes repeated checks downstream
* Allows decoding code to assume correctness

---

### 0.2 Make endian explicit in data types

Introduce:

```rust
struct RawData {
    bytes: Vec<u8>,
    endian: Endian,
}
```

Replace all `Vec<u8>` data fields with `RawData`.

**Effect**

* Eliminates hidden endian dependency
* Makes invalid state unrepresentable

---

## Phase 1 — Data flow correction (core refactor)

### 1.1 Separate *raw* vs *decoded* at the type level

Introduce:

```rust
enum DataState {
    Raw(RawData),
    DecodedF32(Vec<f32>),
    DecodedI16(Vec<i16>),
    DecodedComplex(Vec<Complex<f32>>),
}
```

Stored inside `MrcFile`.

**Rules**

* Only one decoded variant exists at a time
* Raw is dropped once decoded (optional but recommended)

---

### 1.2 Centralize decoding logic

Create a single decode entry point:

```rust
impl MrcFile {
    fn decode_into(&mut self, target: DecodeTarget) -> Result<()>
}
```

Where:

```rust
enum DecodeTarget {
    F32,
    I16,
    ComplexF32,
}
```

**Effect**

* No more duplicated mode checks
* No more scattered decoding logic
* Makes caching trivial

---

## Phase 2 — Performance fixes (major gains)

### 2.1 Add decode caching (critical)

Policy:

* First decode → populate cache
* Subsequent access → borrow cached data
* Never decode twice

Example API:

```rust
pub fn data_f32(&mut self) -> Result<&[f32]>
```

Internally:

* If cached → return
* Else decode once → cache → return

---

### 2.2 Fast-path native-endian decoding

When `file_endian == native_endian`:

```rust
use bytemuck::cast_slice;

let slice: &[f32] = cast_slice(&raw.bytes);
```

When not:

* Decode in chunks
* Use `swap_bytes` on primitives

**Effect**

* 5–10× speedup for common cases

---

### 2.3 Replace eager packed decoding with iterators

Instead of:

```rust
Vec::with_capacity(len * 2)
```

Provide:

```rust
fn packed4bit_iter(&self) -> impl Iterator<Item = u8>
```

Decode on demand.

---

## Phase 3 — View layer repair

### 3.1 Views borrow decoded data, not raw bytes

Change `View` to:

```rust
struct View<'a, T> {
    data: &'a [T],
    shape: Shape,
    stride: Stride,
}
```

Views are now:

* Zero-cost
* Zero-copy
* Decode-once, view-many

---

### 3.2 Typed view construction

```rust
fn view_f32(&mut self) -> Result<View<'_, f32>>
```

No mode checks inside views anymore.

---

## Phase 4 — API clarity & documentation

### 4.1 Make decoding explicit in public API

Avoid magic:

Bad:

```rust
file.as_f32()
```

Good:

```rust
file.decode_f32()?;
let view = file.view_f32();
```

This matches user mental models and avoids surprise allocations.

---

# 2. LLM Vibe Coding Doc

*(drop this into your repo as `DESIGN.md` or `VIBE.md`)*

---

## MRC Crate — Data Flow & Decode Philosophy

### Core Principle

> **Raw bytes are not data. Decoded values are data.**
>
> The crate must never blur this boundary.

---

## Mental Model (Read This First)

```
File bytes
  ↓
RawData { bytes, endian }
  ↓ (exactly once)
Decoded<T>
  ↓
Views & analysis
```

* Header is always native-endian
* Data is always file-endian until decoded
* Decoding is **explicit, cached, and irreversible**

---

## Invariants (Do Not Break)

1. Dimensions are validated at open
2. Decoding happens at most once
3. Views never trigger decoding
4. Mode correctness is guaranteed before decoding
5. Endianness is never implicit

If a change violates one of these, it’s wrong.

---

## Data State Machine

```text
┌─────────┐
│  Raw    │
│ (bytes) │
└────┬────┘
     │ decode(target)
     ▼
┌──────────────┐
│ Decoded<T>   │
│ (cached)     │
└──────────────┘
```

No transitions backward.

---

## Performance Rules

* Native-endian → zero-copy cast
* Foreign-endian → chunked decode
* No per-element heap allocation
* No repeated decoding
* No eager expansion of packed formats

---

## How to Add a New Mode

Checklist:

* [ ] Define voxel byte size
* [ ] Define decode target type
* [ ] Implement fast-path cast
* [ ] Implement endian-swap fallback
* [ ] Add invariant validation
* [ ] Add iterator if packed

If any box is skipped, performance will regress.

---

## Common Pitfalls (Avoid These)

❌ Returning `Vec<T>` from public APIs
❌ Decoding inside views
❌ Checking mode in multiple places
❌ Storing endian as global state
❌ Assuming small data sizes

---

## Design Intent (Why This Exists)

This crate targets:

* Cryo-EM
* Tomography
* Large volumetric data

That means:

* Files are **huge**
* Data is accessed **many times**
* Decode cost dominates runtime

We optimize for **amortized cost**, not convenience.

---

## When in Doubt

Ask:

> “Does this cause decoding more than once?”

If yes, redesign.
