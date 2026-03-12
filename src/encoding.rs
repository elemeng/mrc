//! Encoding trait for converting between file bytes and voxel values
//!
//! This module provides a unified trait for handling endianness-aware
//! encoding/decoding of voxel data.

use crate::{FileEndian, Mode, Voxel};
use crate::voxel::{ComplexI16, ComplexF32};

/// Trait for encoding/decoding voxel data with endianness handling
///
/// Each MRC mode has a corresponding encoding that knows:
/// - The mode constant
/// - How to decode bytes to voxels
/// - How to encode voxels to bytes
pub trait Encoding: Voxel {
    /// The MRC mode this encoding handles
    const MODE: Mode;
    
    /// Size of one voxel in bytes
    const SIZE: usize;
    
    /// Decode a single voxel from file-endian bytes
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self;
    
    /// Encode a single voxel to file-endian bytes
    fn encode(self, endian: FileEndian, bytes: &mut [u8]);
}

// ============================================================================
// Encoding implementations using a macro to reduce boilerplate
// ============================================================================

macro_rules! impl_encoding {
    ($ty:ty, $mode:expr, $size:expr, $decode:expr, $encode:expr) => {
        impl Encoding for $ty {
            const MODE: Mode = $mode;
            const SIZE: usize = $size;
            
            #[inline]
            fn decode(endian: FileEndian, bytes: &[u8]) -> Self {
                $decode(endian, bytes)
            }
            
            #[inline]
            fn encode(self, endian: FileEndian, bytes: &mut [u8]) {
                $encode(self, endian, bytes)
            }
        }
    };
}

// i8 encoding (Mode 0)
impl_encoding!(
    i8,
    Mode::Int8,
    1,
    |_endian: FileEndian, bytes: &[u8]| bytes[0] as i8,
    |v: i8, _endian: FileEndian, bytes: &mut [u8]| bytes[0] = v as u8
);

// i16 encoding (Mode 1)
impl_encoding!(
    i16,
    Mode::Int16,
    2,
    |endian: FileEndian, bytes: &[u8]| {
        let arr: [u8; 2] = bytes.try_into().unwrap();
        match endian {
            FileEndian::Little => i16::from_le_bytes(arr),
            FileEndian::Big => i16::from_be_bytes(arr),
        }
    },
    |v: i16, endian: FileEndian, bytes: &mut [u8]| {
        let arr = match endian {
            FileEndian::Little => v.to_le_bytes(),
            FileEndian::Big => v.to_be_bytes(),
        };
        bytes.copy_from_slice(&arr);
    }
);

// f32 encoding (Mode 2)
impl_encoding!(
    f32,
    Mode::Float32,
    4,
    |endian: FileEndian, bytes: &[u8]| {
        let arr: [u8; 4] = bytes.try_into().unwrap();
        match endian {
            FileEndian::Little => f32::from_le_bytes(arr),
            FileEndian::Big => f32::from_be_bytes(arr),
        }
    },
    |v: f32, endian: FileEndian, bytes: &mut [u8]| {
        let arr = match endian {
            FileEndian::Little => v.to_le_bytes(),
            FileEndian::Big => v.to_be_bytes(),
        };
        bytes.copy_from_slice(&arr);
    }
);

// u16 encoding (Mode 6)
impl_encoding!(
    u16,
    Mode::Uint16,
    2,
    |endian: FileEndian, bytes: &[u8]| {
        let arr: [u8; 2] = bytes.try_into().unwrap();
        match endian {
            FileEndian::Little => u16::from_le_bytes(arr),
            FileEndian::Big => u16::from_be_bytes(arr),
        }
    },
    |v: u16, endian: FileEndian, bytes: &mut [u8]| {
        let arr = match endian {
            FileEndian::Little => v.to_le_bytes(),
            FileEndian::Big => v.to_be_bytes(),
        };
        bytes.copy_from_slice(&arr);
    }
);

// ComplexI16 encoding (Mode 3)
impl Encoding for ComplexI16 {
    const MODE: Mode = Mode::Int16Complex;
    const SIZE: usize = 4;
    
    #[inline]
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self {
        let re = <i16 as Encoding>::decode(endian, &bytes[0..2]);
        let im = <i16 as Encoding>::decode(endian, &bytes[2..4]);
        Self::new(re, im)
    }
    
    #[inline]
    fn encode(self, endian: FileEndian, bytes: &mut [u8]) {
        self.re.encode(endian, &mut bytes[0..2]);
        self.im.encode(endian, &mut bytes[2..4]);
    }
}

// ComplexF32 encoding (Mode 4)
impl Encoding for ComplexF32 {
    const MODE: Mode = Mode::Float32Complex;
    const SIZE: usize = 8;
    
    #[inline]
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self {
        let re = <f32 as Encoding>::decode(endian, &bytes[0..4]);
        let im = <f32 as Encoding>::decode(endian, &bytes[4..8]);
        Self::new(re, im)
    }
    
    #[inline]
    fn encode(self, endian: FileEndian, bytes: &mut [u8]) {
        self.re.encode(endian, &mut bytes[0..4]);
        self.im.encode(endian, &mut bytes[4..8]);
    }
}

// f16 encoding (Mode 12)
#[cfg(feature = "f16")]
impl Encoding for half::f16 {
    const MODE: Mode = Mode::Float16;
    const SIZE: usize = 2;
    
    #[inline]
    fn decode(endian: FileEndian, bytes: &[u8]) -> Self {
        let arr: [u8; 2] = bytes.try_into().unwrap();
        let bits = match endian {
            FileEndian::Little => u16::from_le_bytes(arr),
            FileEndian::Big => u16::from_be_bytes(arr),
        };
        half::f16::from_bits(bits)
    }
    
    #[inline]
    fn encode(self, endian: FileEndian, bytes: &mut [u8]) {
        let bits = self.to_bits();
        let arr = match endian {
            FileEndian::Little => bits.to_le_bytes(),
            FileEndian::Big => bits.to_be_bytes(),
        };
        bytes.copy_from_slice(&arr);
    }
}