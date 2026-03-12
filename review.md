  API Design Review: MRC Library

  Executive Summary

  The library demonstrates solid fundamentals with zero-copy design, sealed traits, and good type safety. However,
  several API design issues reduce usability and add unnecessary complexity. Below are categorized findings with
  specific recommendations.

  ---

  1. Overly Fragmented Header Types

  Problem
  The library exposes two header types (RawHeader and Header) with confusing ownership semantics:

- RawHeader: Direct binary mapping (public fields)
- Header: Wraps RawHeader and adds Deref/DerefMut to expose raw fields

  Issues

- User confusion: Which type to use when? Users must understand internal representation.
- Deref anti-pattern: Header uses Deref to expose RawHeader fields, which is idiomatic Rust but creates ambiguity
     about which type owns what.
- Redundant accessors: Both header.nx (via Deref) and header.nx() exist, returning different types (i32 vs usize).

  Recommendation
  Consolidate to a single Header type with explicit accessor methods:

   // Instead of: header.nx (i32, file-endian) vs header.nx() (usize, native)
   pub struct Header { /*private fields*/ }

   impl Header {
       pub fn nx(&self) -> usize { /*validated, native endian */ }
       pub fn set_nx(&mut self, value: usize) { /* converts to file format*/ }
   }

  ---

  1. Inconsistent Error Handling Granularity

  Problem
  Error::InvalidHeader is too coarse. Different validation failures return the same variant:

   // All return Error::InvalidHeader but represent different problems:
   // - Invalid axis map
   // - Invalid label count (nlabl)
   // - Invalid MAP field

  Issues

- Poor debuggability: Users cannot distinguish between different header problems.
- No recovery path: Cannot handle specific validation failures differently.

  Recommendation
  Expand error variants for actionable failure modes:

   pub enum Error {
       InvalidAxisMap,
       InvalidLabelCount { count: i32 },
       InvalidMapField { bytes: [u8; 4] },
       // ... existing variants
   }

  ---

  1. Trait Hierarchy Complexity

  Problem
  Four overlapping access traits create confusion:

   VoxelAccess          - linear indexing, type-parametric get
   VoxelAccessMut       - mutable linear indexing
   VolumeAccess         - 3D access, associated type Voxel
   VolumeAccessMut      - mutable 3D access

  Issues

- Type erasure friction: VoxelAccess::get<T>() requires type parameter at call site, but VolumeAccess::get_at()
     returns Self::Voxel (no choice).
- Redundant methods: Both VoxelAccessMut::fill and VolumeAccessMut::fill_volume do the same thing.
- Confusing relationship: VolumeAccess extends VoxelAccess but they have different type signatures.

  Recommendation
  Simplify to two core traits:

   /// Type-erased access (runtime mode)
   pub trait DynVoxelAccess {
       fn mode(&self) -> Mode;
       fn len(&self) -> usize;
       fn get_f32(&self, index: usize) -> Result<f32, Error>;
       // ... one method per primitive type
   }

   /// Statically typed access (compile-time mode)
   pub trait VolumeAccess {
       type Voxel: Voxel;
       fn get(&self, index: usize) -> Self::Voxel;
       fn get_at(&self, x: usize, y: usize, z: usize) -> Self::Voxel;
       // ... all dimension/stride methods
   }

  ---

  1. Volume Type Has Too Many Generic Parameters

  Problem
  Volume<T, S, const D: usize> has three parameters, but D=1 and D=2 are barely used:

   pub type Volume1D<T, S> = Volume<T, S, 1>;
   pub type Image2D<T, S> = Volume<T, S, 2>;

  Issues

- Cognitive overhead: Users must understand 3 generic parameters.
- Limited value: D=1 and D=2 have incomplete method sets (e.g., get_1d, get_pixel vs get_at).
- Inconsistent naming: get_1d, get_pixel, get_at — all access voxels but named differently.

  Recommendation
  Consider separate types or remove underused variants:

   // Option A: Single 3D volume (remove D parameter)
   pub struct Volume<T, S> { /*always 3D*/ }

   // Option B: Separate types if truly needed
   pub struct Volume3D<T, S> { /*full API */ }
   pub struct Slice2D<'a, T> { /* view into 3D volume*/ }

  ---

  1. Endianness Handling Exposed in Public API

  Problem
  Users must manually pass FileEndian to many methods:

   Volume::from_raw(data, mode, endian)?;
   Volume::from_data(nx, ny, nz, endian, storage)?;

  Issues

- Leaky abstraction: File endianness is an implementation detail that should be handled internally.
- Redundant information: The Header already contains file_endian, but users must pass it separately.

  Recommendation
  Endianness should be derived from header or defaulted:

   // User provides header, endianness comes from it
   Volume::new(header, storage)?

   // For header-less construction, use native endian by default
   Volume::from_data(dimensions, storage)?

  ---

  1. Missing Builder Pattern for Volume Construction

  Problem
  Volume::new() requires all parameters upfront, but Volume::from_data() is incomplete (missing origin, statistics,
  etc.).

  Issues

- Incomplete defaults: Users who want to set origin/statistics must create a Header manually.
- No validation pipeline: Cannot validate dimensions before allocation.

  Recommendation
  Add a builder:

   Volume::builder()
       .dimensions(64, 64, 64)
       .mode(Mode::Float32)
       .origin(0.0, 0.0, 0.0)
       .voxel_size(1.0, 1.0, 1.0)
       .build(data)?

  ---

  1. Legacy Type Aliases Without Clear Migration Path

  Problem
  Two sets of aliases exist:

   pub type Int16Complex = ComplexI16;
   pub type Float32Complex = ComplexF32;

  Issues

- No deprecation notices: Users don't know which to use.
- Inconsistent style: ComplexI16 follows Rust convention (type first), but Int16Complex follows C convention.

  Recommendation
  Deprecate old aliases with clear guidance:

   #[deprecated(since = "0.3.0", note = "Use ComplexI16 instead")]
   pub type Int16Complex = ComplexI16;

  ---

  1. Voxel Trait Hierarchy Has Unused Markers

  Problem
  Marker traits IntegerVoxel and ComplexVoxel define conversions but have limited utility:

   pub trait IntegerVoxel: ScalarVoxel {
       fn from_i64(v: i64) -> Self;
       fn to_i64(self) -> i64;
       // ...
   }

  Issues

- Saturating casts hide errors: from_i64(v) silently saturates, potentially hiding bugs.
- No generic algorithms: No public functions use these bounds.

  Recommendation
  Either:

   1. Add generic algorithms that justify these bounds, or
   2. Remove them and use TryFrom/TryInto standard traits.

  ---

  1. Missing Conversion Traits

  Problem
  No standard From/TryFrom between Volume types:

   // Users must manually convert:
   let f32_volume: Volume<f32,_> = /*...*/;
   let i16_data: Vec<i16> = f32_volume.iter().map(|v| v as i16).collect();

  Recommendation
  Add conversion traits or methods:

   impl<T, U, S> Volume<T, S>
   where
       T: Voxel + Into<U>,
       U: Voxel,
   {
       pub fn convert(&self) -> Volume<U, Vec<u8>> { /*...*/ }
   }

  ---

  1. Inconsistent Naming Patterns

  ┌─────────────────┬─────────────────────────────────────────┬──────────────────────┐
  │ Pattern         │ Examples                                │ Issue                │
  ├─────────────────┼─────────────────────────────────────────┼──────────────────────┤
  │ _checked suffix │ get_checked, get_at_checked, set_at_checked │ Good                 │
  │ *opt suffix     │ get_opt, set_at_opt, set_opt                │ Conflicting naming   │
  │ try* prefix     │ try_new                                 │ Different convention │
  └─────────────────┴─────────────────────────────────────────┴──────────────────────┘

  Recommendation
  Standardize on one pattern:

- _checked → returns Result<T, Error>
- Return Option<T> for simple presence/absence (rename from _opt)

  ---

  1. MrcWriter vs MrcWriterBuilder Redundancy

  Problem
  Two ways to write files:

   // Method 1: Builder
   MrcWriter::builder()
       .dimensions(64, 64, 64)
       .mode(Mode::Float32)
       .data(data)
       .write("out.mrc")?;

   // Method 2: Direct
   let mut writer = MrcWriter::create("out.mrc", header)?;
   writer.write_data(&data)?;

  Issues

- Unclear when to use which: Builder handles simple cases, direct handles complex.
- Builder cannot update statistics: No way to compute stats after setting data.

  Recommendation
  Unify with a single fluent API:

   MrcWriter::create("out.mrc")?
       .dimensions(64, 64, 64)
       .mode(Mode::Float32)
       .origin(0.0, 0.0, 0.0)
       .write_data(&data)?
       .with_computed_stats()  // optional statistics
       .finish()?;

  ---

  1. Public Re-exports Leak Module Structure

  Problem
  In lib.rs:

   pub use voxel::{..., FileEndian, ...};
   pub use header::{Header, HeaderBuilder, RawHeader};

  FileEndian is exported at crate root, but logically belongs to internal endianness handling.

  Issues

- Users shouldn't need FileEndian: It's an implementation detail for cross-platform handling.
- Pollutes documentation: New users see low-level types they don't need.

  Recommendation
  Keep FileEndian internal or make it an implementation detail:

   // Remove from root exports
   // Users interact with header, not endianness directly

  ---

  1. Encoding Trait Duplicates Voxel::MODE

  Problem
  Both Voxel and Encoding have MODE constant:

   pub trait Voxel {
       const MODE: Mode;
   }

   pub trait Encoding: Voxel {
       const MODE: Mode; // Duplicate!
   }

  Issues

- Confusing inheritance: Encoding inherits from Voxel but redeclares MODE.
- Potential for inconsistency: What if they differ?

  Recommendation
  Remove duplicate from Encoding:

   pub trait Encoding: Voxel {
       const SIZE: usize;  // Keep SIZE (unique to Encoding)
       // MODE inherited from Voxel
   }

  ---

  Summary of Recommendations

  ┌──────────┬────────────────────────────┬──────────────────────────────────────┐
  │ Priority │ Issue                      │ Action                               │
  ├──────────┼────────────────────────────┼──────────────────────────────────────┤
  │ High     │ Fragmented header types    │ Consolidate to single Header         │
  │ High     │ Trait hierarchy complexity │ Simplify to 2 traits                 │
  │ Medium   │ Volume generic parameters  │ Remove D parameter or separate types │
  │ Medium   │ Endianness in public API   │ Hide inside header/volume            │
  │ Medium   │ Missing Volume builder     │ Add fluent builder                   │
  │ Low      │ Legacy type aliases        │ Add deprecation notices              │
  │ Low      │ Inconsistent naming        │ Standardize _checked pattern         │
  │ Low      │ Encoding::MODE duplication │ Inherit from Voxel                   │
  └──────────┴────────────────────────────┴──────────────────────────────────────┘
