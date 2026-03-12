//! Core traits for MRC file handling

use crate::{FileEndian, Float32Complex, Int16Complex, Mode};

/// VoxelType trait for MRC voxel data types
///
/// This trait seals the supported types for voxel data and provides
/// compile-time mode checking. It's used by the unified DataBlock API
/// to ensure type-safe access to voxel data.
///
/// # Safety
/// This trait is sealed - only types defined in this crate can implement it.
pub trait VoxelType: DecodeFromFile + Copy + 'static {
    /// The MRC mode corresponding to this type
    const MODE: Mode;

    /// Check if a given mode is valid for this type
    fn is_valid_mode(mode: Mode) -> bool {
        mode == Self::MODE
    }
}

// Implement VoxelType for all supported voxel types

impl VoxelType for i8 {
    const MODE: Mode = Mode::Int8;
}

impl VoxelType for i16 {
    const MODE: Mode = Mode::Int16;
}

impl VoxelType for f32 {
    const MODE: Mode = Mode::Float32;
}

impl VoxelType for Int16Complex {
    const MODE: Mode = Mode::Int16Complex;
}

impl VoxelType for Float32Complex {
    const MODE: Mode = Mode::Float32Complex;
}

impl VoxelType for u16 {
    const MODE: Mode = Mode::Uint16;
}

#[cfg(feature = "f16")]
impl VoxelType for half::f16 {
    const MODE: Mode = Mode::Float16;
}

// Note: Packed4Bit is not a VoxelType because it packs two values per byte,
// making it incompatible with the unified slice-based API.

/// Decode typed values from raw file bytes with correct endianness
///
/// This is the ONLY place where file endianness conversion happens.
/// Raw bytes are always in file endian, returned values are always native endian.
pub trait DecodeFromFile: Sized {
    /// Size of this type in bytes
    const SIZE: usize;

    /// Decode from raw bytes, converting from file endian to native endian
    fn decode(file_endian: FileEndian, bytes: &[u8]) -> Self;
}

/// Encode typed values to raw file bytes with correct endianness
///
/// This is the ONLY place where file endianness conversion happens.
/// Input values are always native endian, output bytes are always file endian.
pub trait EncodeToFile {
    /// Size of this type in bytes
    const SIZE: usize;

    /// Encode to raw bytes, converting from native endian to file endian
    fn encode(self, file_endian: FileEndian, out: &mut [u8]);
}

// Implementations for primitive types used in MRC files

impl DecodeFromFile for i32 {
    const SIZE: usize = 4;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 4] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => i32::from_le_bytes(arr),
            FileEndian::BigEndian => i32::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for i32 {
    const SIZE: usize = 4;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for f32 {
    const SIZE: usize = 4;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 4] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => f32::from_le_bytes(arr),
            FileEndian::BigEndian => f32::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for f32 {
    const SIZE: usize = 4;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for i16 {
    const SIZE: usize = 2;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 2] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => i16::from_le_bytes(arr),
            FileEndian::BigEndian => i16::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for i16 {
    const SIZE: usize = 2;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for u16 {
    const SIZE: usize = 2;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 2] = b.try_into().unwrap();
        match e {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        }
    }
}

impl EncodeToFile for u16 {
    const SIZE: usize = 2;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bytes = match e {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}

impl DecodeFromFile for i8 {
    const SIZE: usize = 1;

    fn decode(_e: FileEndian, b: &[u8]) -> Self {
        b[0] as i8
    }
}

impl EncodeToFile for i8 {
    const SIZE: usize = 1;

    fn encode(self, _e: FileEndian, out: &mut [u8]) {
        out[0] = self as u8;
    }
}

// Optional f16 support (Mode 12)

#[cfg(feature = "f16")]
impl DecodeFromFile for half::f16 {
    const SIZE: usize = 2;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let arr: [u8; 2] = b.try_into().unwrap();
        let bits = match e {
            FileEndian::LittleEndian => u16::from_le_bytes(arr),
            FileEndian::BigEndian => u16::from_be_bytes(arr),
        };
        half::f16::from_bits(bits)
    }
}

#[cfg(feature = "f16")]
impl EncodeToFile for half::f16 {
    const SIZE: usize = 2;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let bits = self.to_bits();
        let bytes = match e {
            FileEndian::LittleEndian => bits.to_le_bytes(),
            FileEndian::BigEndian => bits.to_be_bytes(),
        };
        out.copy_from_slice(&bytes);
    }
}
