# MRC Conversion Implementation Gaps

This document tracks the gaps between the current implementation and the conversion matrix specification.

## Legend

- ✅ **Implemented** - Fully compliant with matrix
- ⚠️ **Partial** - Implemented but needs enhancement
- ❌ **Missing** - Not yet implemented
- 🚫 **Intentionally Omitted** - Deliberately not implemented per spec

## Conversion Matrix Status

### Tier 1: Must-Have Conversions (Critical for Workflows)

| Conversion | Status | Notes |
|------------|--------|-------|
| M101 → M6 (`unpack_to_u16`) | ❌ | **Critical for K2/K3 workflows**. Batch unpack 4-bit packed data to u16. Handle odd-width row padding. |
| M101 → M0 (`unpack_to_i8`) | ⚠️ | Partial: `Convert<Packed4Bit>` for i8 exists but only gets first nibble. Needs batch function. |
| M101 → M2 (`unpack_to_f32`) | ❌ | **Critical for K2/K3 workflows**. Direct unpack to f32 for processing pipelines. |
| M6 → M2 (`into_f32`) | ✅ | `impl Convert<u16> for f32` + SIMD batch |
| M0/M1 → M2 | ✅ | `impl Convert<i8/i16> for f32` + SIMD batch |
| M12 → M2 | ✅ | `impl Convert<f16> for f32` (feature-gated) |

### Tier 2: Workflow Optimizations

| Conversion | Status | Notes |
|------------|--------|-------|
| M2 → M6 (`try_into_u16`) | ⚠️ | Clamp-based conversion exists. Needs pre-flight `CheckedConvert` trait with range validation. |
| M2 → M12 (`try_into_f16`) | ⚠️ | Cast exists. Needs ±65504 range check via `CheckedConvert`. |
| M2 → M1 (`try_into_i16`) | ⚠️ | Clamp-based conversion exists. Needs `CheckedConvert` for validation. |
| M4 → M2 (Complex→Real) | ❌ | **Missing strategy enum**. Need `ComplexToRealStrategy` {Real, Imag, Magnitude, Phase}. |
| M6 → M0 | ⚠️ | Clamp conversion exists but loses 8 bits. Should require explicit `quantize_8bit()` call. |

### Tier 3: Edge Cases & Legacy

| Conversion | Status | Notes |
|------------|--------|-------|
| M3 → M4 | 🔒 | Read-only implemented. Deprecation warning on write. |
| M3 → Others | 🚫 | Correctly not implemented per spec. |
| M4 → M3 | 🚫 | Should be prohibited. Currently returns clamped i16. |
| Real ↔ Complex | ⚠️ | `Convert` impls exist. Missing explicit strategy requirement. |
| Any → M101 | 🚫 | **Correctly not implemented** (data destruction). |

## Critical Safety Rules Compliance

### The M101 (4-bit) Rule

**Status**: ⚠️ Partial

**Current**:
```rust
impl Convert<Packed4Bit> for u8 {
    fn convert(src: Packed4Bit) -> Self { src.first() }  // Only first nibble!
}
```

**Required**:
```rust
pub fn unpack_u4_to_u16(src: &[Packed4Bit], num_values: usize) -> Vec<u16>;
pub fn unpack_u4_to_f32(src: &[Packed4Bit], num_values: usize) -> Vec<f32>;
pub fn unpack_u4_bytes_to_u16(src: &[u8], num_values: usize) -> Vec<u16>;
pub fn unpack_u4_bytes_to_f32(src: &[u8], num_values: usize) -> Vec<f32>;
```

**Impact**: Blocking K2/K3 camera workflows.

### The Mode 0 Ambiguity

**Status**: ❌ Missing

**Required**:
```rust
pub enum M0Interpretation { Signed, Unsigned }
pub fn reinterpret_m0(data: &[u8], interp: M0Interpretation) -> Vec<f32>;
```

**Impact**: Cannot correctly read legacy pre-MRC2014 files.

### Complex Number Semantics

**Status**: ❌ Missing

**Required**:
```rust
pub enum ComplexToRealStrategy {
    RealPart, ImaginaryPart, Magnitude, Phase
}
impl Float32Complex {
    pub fn to_real(&self, strategy: ComplexToRealStrategy) -> f32;
}
```

**Impact**: Mode 4 → Mode 2 conversions have undefined semantics.

## Trait Design Gaps

### Current

```rust
pub trait Convert<S>: Sized {
    fn convert(src: S) -> Self;
}
```

### Required

```rust
pub trait Convert<S>: Sized {
    fn convert(src: S) -> Self;
}

pub trait CheckedConvert<S>: Sized {
    fn try_convert(src: S) -> Result<Self, ConversionError>;
    fn check_range(src: &[S]) -> RangeCheck;
}

pub struct RangeCheck {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub values_out_of_range: usize,
    pub total_values: usize,
}

pub enum ConversionError {
    OutOfRange { min: f64, max: f64, target_min: f64, target_max: f64 },
    NaNValue,
    InfinityValue,
    MissingComplexStrategy,
    ObsoleteMode3,
    PackingInto4Bit,
}
```

## Error Type Gaps

**Current**: Generic `Error` enum with IO/Header/Bounds variants.

**Required**: Specific `ConversionError` for type conversion failures with:
- Range violation details
- NaN/Infinity detection
- Strategy missing errors
- Obsolete mode warnings

## Implementation Priority

### P0 (Blocking)
1. M101 unpacking functions (`unpack_u4_to_u16`, `unpack_u4_to_f32`)
2. `CheckedConvert` trait for range validation

### P1 (Important)
3. `ComplexToRealStrategy` enum and conversions
4. `M0Interpretation` for legacy files
5. `ConversionError` type

### P2 (Nice to Have)
6. Mode 3 write deprecation warning
7. f16 ±65504 range validation

## Files Requiring Changes

- `src/engine/convert.rs` - Core conversions, new traits
- `src/mode.rs` - New enums (ComplexToRealStrategy, M0Interpretation)
- `src/error.rs` - ConversionError variants
- `src/writer.rs` - Mode 3 deprecation warning
- `src/lib.rs` - Export new types

## Test Coverage Gaps

- M101 unpacking with odd-width rows
- Complex to real with all strategies
- CheckedConvert range validation
- M0 signed vs unsigned interpretation
- ConversionError conditions
