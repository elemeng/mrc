//! SIMD-accelerated encoding

use crate::endian::FileEndian;
use crate::mode::{Int16Complex, Float32Complex};

pub trait Encode {
    const BYTE_SIZE: usize;
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian);
}

impl Encode for i8 {
    const BYTE_SIZE: usize = 1;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, _endian: FileEndian) {
        bytes[offset] = *self as u8;
    }
}

impl Encode for i16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

impl Encode for u16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

impl Encode for i32 {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 4].copy_from_slice(&arr);
    }
}

impl Encode for f32 {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let arr = match endian {
            FileEndian::LittleEndian => self.to_le_bytes(),
            FileEndian::BigEndian => self.to_be_bytes(),
        };
        bytes[offset..offset + 4].copy_from_slice(&arr);
    }
}

impl Encode for Int16Complex {
    const BYTE_SIZE: usize = 4;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.real.encode(bytes, offset, endian);
        self.imag.encode(bytes, offset + 2, endian);
    }
}

impl Encode for Float32Complex {
    const BYTE_SIZE: usize = 8;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        self.real.encode(bytes, offset, endian);
        self.imag.encode(bytes, offset + 4, endian);
    }
}

#[cfg(feature = "f16")]
impl Encode for f16 {
    const BYTE_SIZE: usize = 2;
    #[inline]
    fn encode(&self, bytes: &mut [u8], offset: usize, endian: FileEndian) {
        let bits = self.to_bits();
        let arr = match endian {
            FileEndian::LittleEndian => bits.to_le_bytes(),
            FileEndian::BigEndian => bits.to_be_bytes(),
        };
        bytes[offset..offset + 2].copy_from_slice(&arr);
    }
}

/// Encode slice with SIMD (generic)
#[cfg(feature = "std")]
pub fn encode_slice<T: Encode>(values: &[T], bytes: &mut [u8], endian: FileEndian) {
    assert_eq!(values.len() * T::BYTE_SIZE, bytes.len());

    for (i, val) in values.iter().enumerate() {
        val.encode(bytes, i * T::BYTE_SIZE, endian);
    }
}