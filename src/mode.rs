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

    /// Returns a sequential index (0-7) for conversion matrix lookup.
    /// This maps the 8 MRC modes to compact array indices.
    #[inline]
    pub fn index(&self) -> usize {
        match self {
            Self::Int8 => 0,
            Self::Int16 => 1,
            Self::Float32 => 2,
            Self::Int16Complex => 3,
            Self::Float32Complex => 4,
            Self::Uint16 => 5,
            Self::Float16 => 6,
            Self::Packed4Bit => 7,
        }
    }

    /// Returns the mode from a sequential index (inverse of index()).
    #[inline]
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Int8),
            1 => Some(Self::Int16),
            2 => Some(Self::Float32),
            3 => Some(Self::Int16Complex),
            4 => Some(Self::Float32Complex),
            5 => Some(Self::Uint16),
            6 => Some(Self::Float16),
            7 => Some(Self::Packed4Bit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Int16Complex {
    pub real: i16,
    pub imag: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Float32Complex {
    pub real: f32,
    pub imag: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Packed4Bit(u8);

impl Packed4Bit {
    /// Create a new Packed4Bit value
    pub fn new(value: u8) -> Self {
        Self(value)
    }

    pub fn first(&self) -> u8 {
        self.0 & 0x0F
    }

    pub fn second(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }
}



/// Trait for MRC voxel types with compile-time mode tracking.
///
/// Each voxel type knows its MRC mode constant, enabling the conversion matrix
/// to dispatch kernels without runtime branching.
pub trait Voxel: Copy + Send + Sync + 'static {
    /// The MRC mode constant for this voxel type
    const MODE: Mode;
    /// Size in bytes for one voxel value
    const BYTE_SIZE: usize;
}

impl Voxel for i8 {
    const MODE: Mode = Mode::Int8;
    const BYTE_SIZE: usize = 1;
}

impl Voxel for i16 {
    const MODE: Mode = Mode::Int16;
    const BYTE_SIZE: usize = 2;
}

impl Voxel for f32 {
    const MODE: Mode = Mode::Float32;
    const BYTE_SIZE: usize = 4;
}

impl Voxel for Int16Complex {
    const MODE: Mode = Mode::Int16Complex;
    const BYTE_SIZE: usize = 4;
}

impl Voxel for Float32Complex {
    const MODE: Mode = Mode::Float32Complex;
    const BYTE_SIZE: usize = 8;
}

impl Voxel for u16 {
    const MODE: Mode = Mode::Uint16;
    const BYTE_SIZE: usize = 2;
}

#[cfg(feature = "f16")]
impl Voxel for f16 {
    const MODE: Mode = Mode::Float16;
    const BYTE_SIZE: usize = 2;
}

impl Voxel for Packed4Bit {
    const MODE: Mode = Mode::Packed4Bit;
    const BYTE_SIZE: usize = 1;
}

// Implement EndianCodec directly - this provides both decode and encode
impl crate::engine::codec::EndianCodec for Packed4Bit {
    const BYTE_SIZE: usize = 1;
    
    #[inline]
    fn from_bytes(bytes: &[u8], offset: usize, endian: crate::FileEndian) -> Self {
        let byte = bytes[offset];
        Self(if endian == crate::FileEndian::LittleEndian {
            byte
        } else {
            byte.reverse_bits()
        })
    }
    
    #[inline]
    fn to_bytes(&self, bytes: &mut [u8], offset: usize, endian: crate::FileEndian) {
        bytes[offset] = if endian == crate::FileEndian::LittleEndian {
            self.0
        } else {
            self.0.reverse_bits()
        };
    }
}
