# an encoding/decoding engine for MRC including the full pipeline

```text
raw bytes ↔ endian-normalized bytes ↔ typed voxel values ↔ converted voxel type
```

with **zero-copy fast paths whenever possible**.
This is the **correct architectural boundary** for a robust, `std`-quality Rust crate.

---

# Final Architecture: Unified Encoding / Decoding Engine

The engine should operate as a **bidirectional pipeline**:

```text
RAW BYTES
   ↓
Endian normalization
   ↓
Typed voxel values
   ↓
Type conversion
```

and the reverse for writing.

```text
Typed voxels
   ↓
Type conversion
   ↓
Endian encoding
   ↓
RAW BYTES
```

Both directions use the **same conversion graph and kernels**.

---

# 1. Four Fundamental Layers

A correct design separates **four transformation layers**.

```text
Layer 1: Raw bytes
Layer 2: Endian-normalized bytes
Layer 3: Typed voxel values
Layer 4: Converted voxel values
```

Pipeline:

```text
bytes → endian → typed → converted
```

Reverse for encoding.

This separation enables:

```text
zero-copy
SIMD fusion
parallelization
minimal allocations
```

---

# 2. Raw Byte Layer

This layer interacts with the file backend:

```text
mmap
file read
stream
async source
```

Output type:

```rust
&[u8]
```

No interpretation yet.

---

# 3. Endian Normalization Layer

Binary formats store values with **file endianness**.

The engine converts them to **native endian values**.

Example:

```rust
fn decode_i16(bytes: &[u8], big: bool) -> i16 {
    let arr = [bytes[0], bytes[1]];
    if big {
        i16::from_be_bytes(arr)
    } else {
        i16::from_le_bytes(arr)
    }
}
```

But **this step disappears entirely when endianness matches**.

---

# 4. Typed Value Layer

After endian normalization we obtain **typed voxels**.

Examples:

```rust
&i16
&u16
&f32
&ComplexI16
```

At this stage the data has correct **semantic meaning**.

---

# 5. Type Conversion Layer

The final layer converts voxel types when needed.

Example conversions:

```text
Int16 → Float32
Uint16 → Float32
Float16 → Float32
Int8 → Float32
Float32 → Int16
```

Using the **type-level conversion graph**:

```rust
pub trait Convert<S>: Sized {
    fn convert(src: S) -> Self;
}
```

Kernel:

```rust
fn convert_slice<S, D>(src: &[S], dst: &mut [D])
where
    D: Convert<S>,
{
    for (s, d) in src.iter().zip(dst.iter_mut()) {
        *d = D::convert(*s);
    }
}
```

---

# 6. Zero-Copy Fast Paths

The engine should detect when operations can be skipped.

### Case 1 — Perfect Match

```text
src_mode == dst_mode
AND
file_endian == native
```

Pipeline collapses to:

```text
bytes → typed slice
```

Cost:

```text
0 copies
0 conversions
0 SIMD
```

---

### Case 2 — Endian Match Only

```text
file_endian == native
src_mode != dst_mode
```

Pipeline:

```text
typed → convert
```

No endian work.

---

### Case 3 — Endian + Convert

```text
file_endian != native
```

Pipeline:

```text
bytes → swap → convert
```

Often fused into one kernel.

---

### Case 4 — Packed Modes

Packed formats require decoding:

```text
Packed4Bit
```

Pipeline:

```text
bytes → unpack → typed → convert
```

Zero-copy is impossible.

---

# 7. Kernel Fusion

For performance, operations should be fused.

Instead of:

```text
swap → store
convert → store
```

Use:

```text
load → swap → convert → store
```

This improves cache behavior and SIMD efficiency.

---

# 8. SIMD Integration

Rust ≥1.85 `std::simd` enables portable vectorization.

Example:

```rust
use std::simd::Simd;

let v = Simd::<i16,16>::from_slice(chunk);
let f = v.cast::<f32>();
```

Typical SIMD targets:

```text
Int8 → Float32
Int16 → Float32
Uint16 → Float32
Float16 → Float32
```

---

# 9. Parallel Execution

Large volumes should run in parallel.

```rust
use rayon::prelude::*;

dst.par_chunks_mut(CHUNK)
   .zip(src.par_chunks(CHUNK))
   .for_each(|(d,s)| convert(s,d));
```

Recommended chunk size:

```text
1–4 MB
```

This balances:

```text
cache locality
thread scheduling
memory bandwidth
```

---

# 10. Encode / Decode Symmetry

The engine should support both directions.

Decode:

```text
bytes → typed → converted
```

Encode:

```text
typed → converted → bytes
```

Using the **same kernels reversed**.

---

# 11. Final Runtime Pipeline

The runtime system only needs to determine:

```text
source voxel type
destination voxel type
file endian
```

Then run the correct pipeline.

Example flow:

```text
read file header
↓
detect voxel mode
↓
detect endian
↓
select kernel
↓
parallel SIMD conversion
```

---

# 12. Resulting System

Final architecture:

```text
RAW BYTES
   ↓
Endian normalization (optional)
   ↓
Typed voxel values
   ↓
Type conversion (optional)
   ↓
Parallel SIMD processing
```

Reverse for encoding.

---

# 13. Key Properties

This architecture guarantees:

```text
zero unsafe code
zero-copy fast paths
minimal runtime branching
SIMD acceleration
parallel processing
complete format coverage
```

Typical implementation size:

```text
~300–500 LOC
```

Yet it supports:

```text
all MRC modes
encoding
decoding
SIMD
rayon
zero-copy
```

---
