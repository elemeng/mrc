#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    Int8 = 0,
    Int16 = 1,
    Float32 = 2,
    /// Complex number with 16-bit integer components (Mode 3)
    ///
    /// # Byte Order
    /// The layout is `[real i16 (2 bytes), imag i16 (2 bytes)]` which matches the
    /// de facto standard used by CCP4, IMOD, and other MRC implementations.
    /// This is not explicitly specified in MRC2014 but is universally adopted.
    Int16Complex = 3,
    /// Complex number with 32-bit float components (Mode 4)
    ///
    /// # Byte Order
    /// The layout is `[real f32 (4 bytes), imag f32 (4 bytes)]` which matches the
    /// de facto standard used by CCP4, IMOD, and other MRC implementations.
    /// This is not explicitly specified in MRC2014 but is universally adopted.
    Float32Complex = 4,
    Uint16 = 6,
    Float16 = 12,
    /// 4-bit data packed two values per byte (mode 101)
    Packed4Bit = 101,
}

impl Mode {
    #[inline]
    pub fn from_i32(mode: i32) -> Option<Self> {
        match mode {
            0 => Some(Self::Int8),
            1 => Some(Self::Int16),
            2 => Some(Self::Float32),
            3 => Some(Self::Int16Complex),
            4 => Some(Self::Float32Complex),
            6 => Some(Self::Uint16),
            12 => Some(Self::Float16),
            101 => Some(Self::Packed4Bit),
            _ => None,
        }
    }

    #[inline]
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Int8 => 1,
            Self::Int16 => 2,
            Self::Float32 => 4,
            Self::Int16Complex => 4,   // 2 bytes real + 2 bytes imaginary
            Self::Float32Complex => 8, // 4 bytes real + 4 bytes imaginary
            Self::Uint16 => 2,
            Self::Float16 => 2,
            Self::Packed4Bit => 1, // 4 bits per value, 2 values per byte
        }
    }

    #[inline]
    pub fn is_complex(&self) -> bool {
        matches!(self, Self::Int16Complex | Self::Float32Complex)
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::Int8 | Self::Int16 | Self::Int16Complex | Self::Uint16 | Self::Packed4Bit
        )
    }

    #[inline]
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float32 | Self::Float32Complex | Self::Float16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Int16Complex {
    pub real: i16,
    pub imag: i16,
}

impl Int16Complex {
    pub fn decode(endian: crate::endian::FileEndian, bytes: &[u8]) -> Self {
        use crate::decode::decode_i16;
        Self {
            real: decode_i16(bytes, 0, endian),
            imag: decode_i16(bytes, 2, endian),
        }
    }

    pub fn encode(self, endian: crate::endian::FileEndian, out: &mut [u8]) {
        use crate::encode::encode_i16;
        encode_i16(self.real, out, 0, endian);
        encode_i16(self.imag, out, 2, endian);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Float32Complex {
    pub real: f32,
    pub imag: f32,
}

impl Float32Complex {
    pub fn decode(endian: crate::endian::FileEndian, bytes: &[u8]) -> Self {
        use crate::decode::decode_f32;
        Self {
            real: decode_f32(bytes, 0, endian),
            imag: decode_f32(bytes, 4, endian),
        }
    }

    pub fn encode(self, endian: crate::endian::FileEndian, out: &mut [u8]) {
        use crate::encode::encode_f32;
        encode_f32(self.real, out, 0, endian);
        encode_f32(self.imag, out, 4, endian);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Packed4Bit(u8);

impl Packed4Bit {
    pub fn decode(endian: crate::endian::FileEndian, bytes: &[u8]) -> Self {
        Self(if endian == crate::endian::FileEndian::LittleEndian {
            bytes[0]
        } else {
            bytes[0].reverse_bits()
        })
    }

    pub fn first(&self) -> u8 {
        self.0 & 0x0F
    }

    pub fn second(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }

    pub fn encode(self, endian: crate::endian::FileEndian, out: &mut [u8]) {
        out[0] = if endian == crate::endian::FileEndian::LittleEndian {
            self.0
        } else {
            self.0.reverse_bits()
        };
    }
}
