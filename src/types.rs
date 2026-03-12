//! Complex number types and Packed4Bit for MRC files

use crate::{DecodeFromFile, EncodeToFile, FileEndian};

// Complex number types for MRC modes 3 and 4

/// Complex number with 16-bit integer components (Mode 3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Int16Complex {
    pub real: i16,
    pub imag: i16,
}

impl DecodeFromFile for Int16Complex {
    const SIZE: usize = 4;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let real_arr: [u8; 2] = b[0..2].try_into().unwrap();
        let imag_arr: [u8; 2] = b[2..4].try_into().unwrap();
        Self {
            real: match e {
                FileEndian::LittleEndian => i16::from_le_bytes(real_arr),
                FileEndian::BigEndian => i16::from_be_bytes(real_arr),
            },
            imag: match e {
                FileEndian::LittleEndian => i16::from_le_bytes(imag_arr),
                FileEndian::BigEndian => i16::from_be_bytes(imag_arr),
            },
        }
    }
}

impl EncodeToFile for Int16Complex {
    const SIZE: usize = 4;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let real_bytes = match e {
            FileEndian::LittleEndian => self.real.to_le_bytes(),
            FileEndian::BigEndian => self.real.to_be_bytes(),
        };
        let imag_bytes = match e {
            FileEndian::LittleEndian => self.imag.to_le_bytes(),
            FileEndian::BigEndian => self.imag.to_be_bytes(),
        };
        out[0..2].copy_from_slice(&real_bytes);
        out[2..4].copy_from_slice(&imag_bytes);
    }
}

/// Complex number with 32-bit float components (Mode 4)
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Float32Complex {
    pub real: f32,
    pub imag: f32,
}

impl DecodeFromFile for Float32Complex {
    const SIZE: usize = 8;

    fn decode(e: FileEndian, b: &[u8]) -> Self {
        let real_arr: [u8; 4] = b[0..4].try_into().unwrap();
        let imag_arr: [u8; 4] = b[4..8].try_into().unwrap();
        Self {
            real: match e {
                FileEndian::LittleEndian => f32::from_le_bytes(real_arr),
                FileEndian::BigEndian => f32::from_be_bytes(real_arr),
            },
            imag: match e {
                FileEndian::LittleEndian => f32::from_le_bytes(imag_arr),
                FileEndian::BigEndian => f32::from_be_bytes(imag_arr),
            },
        }
    }
}

impl EncodeToFile for Float32Complex {
    const SIZE: usize = 8;

    fn encode(self, e: FileEndian, out: &mut [u8]) {
        let real_bytes = match e {
            FileEndian::LittleEndian => self.real.to_le_bytes(),
            FileEndian::BigEndian => self.real.to_be_bytes(),
        };
        let imag_bytes = match e {
            FileEndian::LittleEndian => self.imag.to_le_bytes(),
            FileEndian::BigEndian => self.imag.to_be_bytes(),
        };
        out[0..4].copy_from_slice(&real_bytes);
        out[4..8].copy_from_slice(&imag_bytes);
    }
}

// Packed 4-bit data (Mode 101)
// Two 4-bit values are packed into a single byte

/// Packed 4-bit values (Mode 101)
///
/// Two 4-bit values (0-15) are packed into a single byte.
/// The lower 4 bits contain the first value, the upper 4 bits contain the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packed4Bit {
    pub values: [u8; 2], // Each value is 0-15
}

impl Packed4Bit {
    /// Create a new packed 4-bit value
    ///
    /// # Panics
    /// Panics if either value is greater than 15
    pub fn new(first: u8, second: u8) -> Self {
        assert!(first <= 15, "First value must be 0-15");
        assert!(second <= 15, "Second value must be 0-15");
        Self {
            values: [first, second],
        }
    }

    /// Get the first (lower) 4-bit value
    pub fn first(&self) -> u8 {
        self.values[0]
    }

    /// Get the second (upper) 4-bit value
    pub fn second(&self) -> u8 {
        self.values[1]
    }
}

impl DecodeFromFile for Packed4Bit {
    const SIZE: usize = 1;

    fn decode(_e: FileEndian, b: &[u8]) -> Self {
        let byte = b[0];
        Self {
            values: [byte & 0x0F, (byte >> 4) & 0x0F],
        }
    }
}

impl EncodeToFile for Packed4Bit {
    const SIZE: usize = 1;

    fn encode(self, _e: FileEndian, out: &mut [u8]) {
        out[0] = self.values[0] | (self.values[1] << 4);
    }
}
