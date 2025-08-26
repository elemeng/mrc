use core::f32;

#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Header {
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
    pub mode: i32,
    pub nxstart: i32,
    pub nystart: i32,
    pub nzstart: i32,
    pub mx: i32,
    pub my: i32,
    pub mz: i32,
    pub xlen: f32,  // Unit cell edge length along X (Å).
    pub ylen: f32,  // Unit cell edge length along Y (Å).
    pub zlen: f32,  // Unit cell edge length along Z (Å).
    pub alpha: f32, // Angle between Y and Z axes (degrees).
    pub beta: f32,  // Angle between X and Z axes (degrees).
    pub gamma: f32, // Angle between X and Y axes (degrees).
    pub mapc: i32,  // 1-based index of column axis (1 = X, 2 = Y, 3 = Z).
    pub mapr: i32,  // 1-based index of row axis.
    pub maps: i32,  // 1-based index of section axis.
    pub dmin: f32,  // Minimum density value.
    pub dmax: f32,  // Maximum density value.
    pub dmean: f32, // Mean density value.
    pub ispg: i32,      // Space-group number (1 = P1).
    pub nsymbt: i32,    // Bytes of symmetry data following the header.
    pub extra: [u8; 100], // Reserved; bytes 8–11 hold EXTTYP, 12–15 NVERSION.
    pub origin: [f32; 3], // Volume origin in voxels.
    pub map: [u8; 4],     // Magic bytes “MAP ”.
    pub machst: [u8; 4],  // Machine stamp (little-endian: 0x44 0x44 0x00 0x00).
    pub rms: f32,         // RMS deviation of density values.
    pub nlabl: i32,       // Number of valid labels (0–10).
    pub label: [u8; 800], // Ten 80-byte text labels.
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Header {
    #[inline]
    /// Constructs a default header suitable for a new MRC file.
    ///
    /// All dimensions are zero, the mode is 32-bit float, and
    /// cell angles are 90°. Other fields are set to safe neutral values.
    pub const fn new() -> Self {
        Self {
            nx: 0,
            ny: 0,
            nz: 0,
            mode: 2, // 32-bit floating point
            nxstart: 0,
            nystart: 0,
            nzstart: 0,
            mx: 0,
            my: 0,
            mz: 0,
            xlen: 1.0, // Avoid division by zero.
            ylen: 1.0,
            zlen: 1.0,
            alpha: 90.0,
            beta: 90.0,
            gamma: 90.0,
            mapc: 1, // Column → X
            mapr: 2, // Row    → Y
            maps: 3, // Section→ Z
            dmin: f32::NEG_INFINITY,
            dmax: f32::INFINITY,
            dmean: 0.0,
            ispg: 1, // P1 space group.
            nsymbt: 0,
            extra: [0; 100],
            origin: [0.0; 3],
            map: *b"MAP ",
            machst: [0x44, 0x44, 0x00, 0x00], // Little-endian x86/AMD64.
            rms: 0.0,
            nlabl: 0,
            label: [0; 800],
        }
    }

    #[inline]
    /// Offset, in bytes, from file start to the first voxel value.
    pub const fn data_offset(&self) -> usize {
        1024 + self.nsymbt as usize
    }

    #[inline]
    /// Size, in bytes, of the voxel data block.
    ///
    /// Returns zero for invalid mode or zero dimensions.
    pub fn data_size(&self) -> usize {
        let n = (self.nx as usize) * (self.ny as usize) * (self.nz as usize);
        let bytes_per_pixel = match self.mode {
            0 | 6 => 1, // i8 / u8
            1 | 3 => 2, // i16 / complex i16
            2 | 4 => 4, // f32 / complex f32
            12 => 2,    // f16
            _ => 0,
        };
        n * bytes_per_pixel
    }

    #[inline]
    /// True when dimensions are positive and mode is supported.
    pub fn validate(&self) -> bool {
        self.nx > 0 && self.ny > 0 && self.nz > 0
            && matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12)
    }

    #[inline]
    /// Reads the 4-byte EXTTYP identifier stored in `extra[8..12]`.
    pub const fn exttyp(&self) -> i32 {
        i32::from_le_bytes([
            self.extra[8],
            self.extra[9],
            self.extra[10],
            self.extra[11],
        ])
    }

    #[inline]
    /// Stores the 4-byte EXTTYP identifier into `extra[8..12]`.
    pub fn set_exttyp(&mut self, value: i32) {
        let bytes = value.to_le_bytes();
        self.extra[8..12].copy_from_slice(&bytes);
    }

    #[inline]
    /// Interprets EXTTYP as an ASCII string.
    pub fn exttyp_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.extra[8..12])
    }

    #[inline]
    /// Sets EXTTYP from a 4-character ASCII string.
    pub fn set_exttyp_str(&mut self, value: &str) -> Result<(), &'static str> {
        if value.len() != 4 {
            return Err("EXTTYP must be exactly 4 characters");
        }
        let bytes = value.as_bytes();
        let int_value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.set_exttyp(int_value);
        Ok(())
    }

    #[inline]
    /// Reads the 4-byte NVERSION number stored in `extra[12..16]`.
    pub const fn nversion(&self) -> i32 {
        i32::from_le_bytes([
            self.extra[12],
            self.extra[13],
            self.extra[14],
            self.extra[15],
        ])
    }

    #[inline]
    /// Stores the 4-byte NVERSION number into `extra[12..16]`.
    pub fn set_nversion(&mut self, value: i32) {
        let bytes = value.to_le_bytes();
        self.extra[12..16].copy_from_slice(&bytes);
    }

    #[inline]
    /// Swaps the endianness of every multi-byte field.
    ///
    /// Call this after reading a big-endian MRC file on a little-endian
    /// machine (or vice-versa).
    pub fn swap_endian(&mut self) {
        macro_rules! swap_field {
            ($field:ident) => {
                self.$field = self.$field.swap_bytes();
            };
        }

        swap_field!(nx);
        swap_field!(ny);
        swap_field!(nz);
        swap_field!(mode);
        swap_field!(nxstart);
        swap_field!(nystart);
        swap_field!(nzstart);
        swap_field!(mx);
        swap_field!(my);
        swap_field!(mz);

        self.xlen = f32::from_bits(self.xlen.to_bits().swap_bytes());
        self.ylen = f32::from_bits(self.ylen.to_bits().swap_bytes());
        self.zlen = f32::from_bits(self.zlen.to_bits().swap_bytes());
        self.alpha = f32::from_bits(self.alpha.to_bits().swap_bytes());
        self.beta = f32::from_bits(self.beta.to_bits().swap_bytes());
        self.gamma = f32::from_bits(self.gamma.to_bits().swap_bytes());

        swap_field!(mapc);
        swap_field!(mapr);
        swap_field!(maps);

        self.dmin = f32::from_bits(self.dmin.to_bits().swap_bytes());
        self.dmax = f32::from_bits(self.dmax.to_bits().swap_bytes());
        self.dmean = f32::from_bits(self.dmean.to_bits().swap_bytes());

        swap_field!(ispg);
        swap_field!(nsymbt);

        let exttyp = self.exttyp().swap_bytes();
        self.set_exttyp(exttyp);

        let nversion = self.nversion().swap_bytes();
        self.set_nversion(nversion);

        for val in &mut self.origin {
            *val = f32::from_bits(val.to_bits().swap_bytes());
        }

        swap_field!(nlabl);
        self.rms = f32::from_bits(self.rms.to_bits().swap_bytes());
    }
}