//! Pipeline utilities for encoding/decoding
//!
//! This module provides conversion path analysis and optimization hints.

use super::endian::FileEndian;
use crate::mode::Mode;

/// Conversion path through the 4-layer pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionPath {
    /// Zero-copy: src_mode == dst_mode && endian == native
    /// Just reinterpret bytes as typed slice
    ZeroCopy,
    /// Endian only: src_mode == dst_mode but endian != native
    /// Swap endian bytes only
    EndianOnly,
    /// Convert only: src_mode != dst_mode but endian == native
    /// Type conversion only
    ConvertOnly,
    /// Full pipeline: endian swap + type conversion
    FullPipeline,
}

/// Determine the optimal conversion path for the given parameters.
#[inline]
pub fn get_conversion_path(
    src_mode: Mode,
    dst_mode: Mode,
    file_endian: FileEndian,
) -> ConversionPath {
    let modes_match = src_mode == dst_mode;
    let endian_match = file_endian.is_native();

    match (modes_match, endian_match) {
        (true, true) => ConversionPath::ZeroCopy,
        (true, false) => ConversionPath::EndianOnly,
        (false, true) => ConversionPath::ConvertOnly,
        (false, false) => ConversionPath::FullPipeline,
    }
}

/// Check if zero-copy is possible for the given parameters.
#[inline]
pub fn is_zero_copy(src_mode: Mode, dst_mode: Mode, file_endian: FileEndian) -> bool {
    src_mode == dst_mode && file_endian.is_native()
}
