//! Raw 1024-byte MRC header with exact binary layout
//!
//! This is an internal implementation detail for binary I/O.
//! Users should interact with `Header` instead.

/// Raw 1024-byte MRC header with exact binary layout
///
/// This is an internal type for direct binary I/O only.
/// Field offsets follow the MRC2014 specification.
#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RawHeader {
    // --- Bytes 0-11: Dimensions ---
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,

    // --- Bytes 12-15: Mode ---
    pub mode: i32,

    // --- Bytes 16-27: Start positions ---
    pub nxstart: i32,
    pub nystart: i32,
    pub nzstart: i32,

    // --- Bytes 28-39: Grid sampling ---
    pub mx: i32,
    pub my: i32,
    pub mz: i32,

    // --- Bytes 40-51: Cell dimensions ---
    pub xlen: f32,
    pub ylen: f32,
    pub zlen: f32,

    // --- Bytes 52-63: Cell angles ---
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,

    // --- Bytes 64-75: Axis mapping ---
    pub mapc: i32,
    pub mapr: i32,
    pub maps: i32,

    // --- Bytes 76-87: Statistics ---
    pub dmin: f32,
    pub dmax: f32,
    pub dmean: f32,

    // --- Bytes 88-91: Space group ---
    pub ispg: i32,

    // --- Bytes 92-95: Extended header size ---
    pub nsymbt: i32,

    // --- Bytes 96-195: Extra space (100 bytes) ---
    pub extra: [u8; 100],

    // --- Bytes 196-207: Origin ---
    pub origin: [f32; 3],

    // --- Bytes 208-211: MAP identifier ---
    pub map: [u8; 4],

    // --- Bytes 212-215: Machine stamp ---
    pub machst: [u8; 4],

    // --- Bytes 216-219: RMS ---
    pub rms: f32,

    // --- Bytes 220-223: Label count ---
    pub nlabl: i32,

    // --- Bytes 224-1023: Labels ---
    pub label: [u8; 800],
}

unsafe impl bytemuck::Pod for RawHeader {}
unsafe impl bytemuck::Zeroable for RawHeader {}

impl RawHeader {
    /// Header size in bytes
    #[allow(dead_code)]
    pub const SIZE: usize = 1024;

    /// Create a new header with default values
    pub(crate) fn new() -> Self {
        Self {
            nx: 1,
            ny: 1,
            nz: 1,
            mode: 2, // Float32
            nxstart: 0,
            nystart: 0,
            nzstart: 0,
            mx: 0,
            my: 0,
            mz: 0,
            xlen: 1.0,
            ylen: 1.0,
            zlen: 1.0,
            alpha: 90.0,
            beta: 90.0,
            gamma: 90.0,
            mapc: 1,
            mapr: 2,
            maps: 3,
            dmin: f32::INFINITY,
            dmax: f32::NEG_INFINITY,
            dmean: f32::NEG_INFINITY,
            ispg: 1,
            nsymbt: 0,
            extra: [0; 100],
            origin: [0.0; 3],
            map: *b"MAP ",
            machst: [0x44, 0x44, 0x00, 0x00],
            rms: -1.0,
            nlabl: 0,
            label: [0; 800],
        }
    }

    /// Get the EXTTYP identifier (bytes 96-99, stored in extra[8..12])
    pub(crate) fn exttyp(&self) -> [u8; 4] {
        [self.extra[8], self.extra[9], self.extra[10], self.extra[11]]
    }

    /// Set the EXTTYP identifier
    pub(crate) fn set_exttyp(&mut self, value: [u8; 4]) {
        self.extra[8..12].copy_from_slice(&value);
    }

    /// Get the NVERSION (bytes 100-103, stored in extra[12..16])
    pub(crate) fn nversion(&self, endian: crate::voxel::FileEndian) -> i32 {
        let bytes = [
            self.extra[12],
            self.extra[13],
            self.extra[14],
            self.extra[15],
        ];
        use crate::voxel::EndianConvert;
        i32::from_le_bytes(bytes).convert_from_file(endian)
    }

    /// Set the NVERSION
    pub(crate) fn set_nversion(&mut self, value: i32, endian: crate::voxel::FileEndian) {
        use crate::voxel::EndianConvert;
        let bytes = value.convert_from_file(endian).to_le_bytes();
        self.extra[12..16].copy_from_slice(&bytes);
    }
}

impl Default for RawHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(core::mem::size_of::<RawHeader>(), 1024);
    }
}
