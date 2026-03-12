Below is a **clean final architecture** for a **Rust MRC2014 crate** built specifically around the official spec.
The design goal is:

* **zero redundancy**
* **Zero copy where possible**
* **clear separation of concerns**
* **compile-time safety**
* **high performance (mmap + SIMD friendly)**
* **extensible to cryo-EM pipelines**

No code, only **architecture and trait design**.

---

# 1. Core Design Philosophy

The MRC format actually contains **four orthogonal layers**:

```
        Algorithms
            │
        Volume<T,S,D>
        /     │      \
    Voxel   Storage   Dimensionality
       │
    Encoding
       │
      MODE
```

Where:

| Layer    | Responsibility          |
| -------- | ----------------------- |
| MODE     | file encoding           |
| Encoding | how bytes become voxels |
| Voxel    | numeric semantics       |
| Storage  | memory model            |
| Volume   | user-facing container   |

This **decoupling eliminates duplication**.

---

# 2. Crate Layout

```
mrc
 ├── header
 │     ├── raw_header
 │     ├── validated_header
 │     └── axis_mapping
 │
 ├── mode
 │     ├── mode_enum
 │     └── encoding_traits
 │
 ├── voxel
 │     ├── scalar
 │     ├── complex
 │     └── voxel_traits
 │
 ├── storage
 │     ├── vec_storage
 │     ├── mmap_storage
 │     └── chunk_storage
 │
 ├── volume
 │     ├── volume_core
 │     ├── slicing
 │     └── iterators
 │
 ├── io
 │     ├── reader
 │     ├── writer
 │     └── mode_dispatch
 │
 ├── extended_header
 │     ├── ext_type
 │     └── metadata
 │
 ├── algorithms (optional feature)
 │     ├── stats
 │     ├── normalization
 │     └── fft
 │
 └── error
```

Everything flows **downwards**, preventing circular dependencies.

---

# 3. Header Representation

The MRC header must be **exact binary layout**.

Use a **two-layer header model**.

### Raw header (binary mapping)

```
RawHeader
```

Properties:

* `#[repr(C)]`
* 1024 bytes
* direct byte read

Purpose:

```
read_from_file -> RawHeader
```

---

### Validated header

```
Header
```

Converted using:

```
TryFrom<RawHeader>
```

Responsibilities:

* validate MAP string
* validate MODE
* parse axis ordering
* normalize values

This avoids polluting algorithms with file-format quirks.

---

# 4. Axis System

MRC axes are **permutable** via:

```
MAPC
MAPR
MAPS
```

Create a dedicated structure:

```
AxisMap
```

Conceptually:

```
AxisMap {
   column_axis,
   row_axis,
   section_axis
}
```

Responsibilities:

* map file order → logical XYZ
* reorder slices

This keeps axis complexity **isolated**.

---

# 5. Mode System

Define a **strict enum** for all modes.

```
Mode
```

Variants:

```
Int8
Int16
Float32
ComplexInt16
ComplexFloat32
UInt16
Float16
Packed4
```

This eliminates **magic numbers**.

---

# 6. Encoding Layer

File encodings convert **bytes → voxel values**.

Trait:

```
Encoding
```

Responsibilities:

```
MODE constant
decode
encode
voxel type mapping
```

Mapping:

| Mode | Encoding           | Voxel        |
| ---- | ------------------ | ------------ |
| 0    | Int8Encoding       | i8           |
| 1    | Int16Encoding      | i16          |
| 2    | Float32Encoding    | f32          |
| 3    | ComplexI16Encoding | Complex<i16> |
| 4    | ComplexF32Encoding | Complex<f32> |
| 6    | UInt16Encoding     | u16          |
| 12   | Float16Encoding    | f16          |
| 101  | Packed4Encoding    | u8           |

Packed mode decodes to **u8**.

This keeps algorithms simple.

---

# 7. Voxel Traits

Algorithms operate on **voxel semantics**, not file encodings.

Base trait:

```
Voxel
```

Requirements:

```
Copy
Send
Sync
'static
```

Capabilities layered via traits:

```
ScalarVoxel
ComplexVoxel
RealVoxel
```

Examples:

Scalar:

```
i8
i16
u16
f16
f32
```

Complex:

```
Complex<i16>
Complex<f32>
```

---

# 8. Volume Container

Core user-facing structure:

```
Volume<T,S,const D: usize>
```

Parameters:

| Parameter | Meaning         |
| --------- | --------------- |
| T         | voxel type      |
| S         | storage backend |
| D         | dimensionality  |

Examples:

```
Volume<f32, VecStorage, 3>
Volume<f32, MmapStorage, 3>
Volume<u8, VecStorage, 2>
```

Benefits:

* compile-time dimension safety
* unified image/volume model

---

# 9. Storage Abstraction

Large cryo-EM maps require flexible storage.

Trait:

```
Storage<T>
```

Responsibilities:

```
shape
data access
mutable access
```

Implementations:

| Storage      | Purpose        |
| ------------ | -------------- |
| VecStorage   | small data     |
| MmapStorage  | large datasets |
| ChunkStorage | streaming      |

Algorithms depend only on:

```
Storage<T>
```

---

# 10. Extended Header System

MRC extended headers vary widely.

Represent them as:

```
ExtendedHeader
```

with a type enum:

```
ExtType
```

Values from spec:

```
CCP4
MRCO
SERI
AGAR
FEI1
FEI2
HDF5
Unknown
```

Parsing strategy:

```
Raw bytes
+ optional typed parser
```

This allows future extension.

---

# 11. Data Block Reader

Reader pipeline:

```
file
 ↓
RawHeader
 ↓
Header validation
 ↓
Mode detection
 ↓
Encoding selection
 ↓
Storage allocation
 ↓
decode data
 ↓
Volume<T>
```

Mode dispatch occurs **once**.

After that everything is generic.

---

# 12. Runtime Type Handling

For unknown modes at compile time:

Use enum wrapper:

```
VolumeData
```

Variants:

```
I8
I16
U16
F16
F32
ComplexI16
ComplexF32
U8
```

This avoids heap allocations from trait objects.

---

# 13. Iteration Model

Volumes expose iterators:

```
voxels()
rows()
sections()
chunks()
```

Parallel iteration enabled via **rayon**.

Large tomograms benefit significantly.

---

# 14. Optional Feature Integrations

Recommended feature flags:

```
features = [
   "ndarray",
   "rayon",
   "mmap",
   "fft"
]
```

Example integration with **ndarray**:

```
Volume -> ArrayView
```

FFT support via **rustfft**.

---

# 15. Error System

Unified error enum:

```
MrcError
```

Categories:

```
InvalidHeader
InvalidMode
InvalidAxisMapping
CorruptFile
UnsupportedFeature
IoError
```

Implement via **thiserror**.

---

# 16. Macro Usage

Macros remove repetitive implementations.

Use `macro_rules!` for:

### voxel trait implementations

```
impl_voxel!(i8);
impl_voxel!(i16);
impl_voxel!(u16);
impl_voxel!(f32);
```

### encoding implementations

```
impl_encoding!(Mode::Int8, Int8Encoding, i8);
```

This keeps the crate **small and maintainable**.

---

# 17. Builder API for Writing

Writing MRC files should use a builder.

Conceptually:

```
MrcWriter::builder()
   .shape(nx,ny,nz)
   .mode(Float32)
   .voxel_size(1.5)
   .origin(...)
   .build()
```

This prevents invalid headers.

---

# 18. Performance Optimizations

Key optimizations:

### memory mapping

Large maps use mmap storage.

### zero-copy header read

Direct struct mapping.

### SIMD-friendly voxel loops

Generics compile to specialized loops.

### parallel iteration

Enabled with **rayon**.

---

# 19. API Style

User-facing API should look like:

```
let map = MrcFile::open("map.mrc")?;

let volume = map.read_volume::<f32>()?;

println!("{}", volume.shape());
```

Or dynamic:

```
let data = map.read()?;
```

---

# 20. Final Result

This architecture achieves:

### minimal redundancy

All algorithms written once.

### correctness

Compile-time voxel types.

### speed

Zero-copy header + parallel iteration.

### extensibility

New MRC modes require only an encoding.

### clarity

Each concept lives in exactly one module.

---

# Final Conceptual Model

```
              MrcFile
                 │
               Header
                 │
              Mode enum
                 │
            Encoding trait
                 │
              Voxel type
                 │
          Volume<T,Storage,D>
                 │
            Algorithms
```

This is essentially the **cleanest architecture possible for an MRC crate**.