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
    pub xlen: f32,  //CEELA: Cell dimensions in Angstroms (Å) along X axes
    pub ylen: f32,  //CEELA: Cell dimensions in Angstroms (Å) along Y axes
    pub zlen: f32,  //CEELA: Cell dimensions in Angstroms (Å) along Z axes
    pub alpha: f32, //CELLB: Cell angles in degrees between the crystallographic axes Y and Z axes
    pub beta: f32,  //CELLB: Cell angles in degrees between the crystallographic axes X and Z axes
    pub gamma: f32, //CELLB: Cell angles in degrees between the crystallographic axes X and Y axes
    pub mapc: i32,
    pub mapr: i32,
    pub maps: i32,
    pub dmin: f32,
    pub dmax: f32,
    pub dmean: f32,
    pub ispg: i32,
    pub nsymbt: i32,
    pub extra: [u8; 100],
    pub origin: [f32; 3],
    pub map: [u8; 4],
    pub machst: [u8; 4],
    pub rms: f32,
    pub nlabl: i32,
    pub label: [u8; 800],
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Header {
    #[inline]
    pub const fn new() -> Self {
        Self {
            nx: 0,
            ny: 0,
            nz: 0,
            mode: 0,
            nxstart: 0,
            nystart: 0,
            nzstart: 0,
            mx: 0,
            my: 0,
            mz: 0,
            xlen: 0.0,
            ylen: 0.0,
            zlen: 0.0,
            alpha: 0.0,
            beta: 0.0,
            gamma: 0.0,
            mapc: 0,
            mapr: 0,
            maps: 0,
            dmin: 0.0,
            dmax: 0.0,
            dmean: 0.0,
            ispg: 0,
            nsymbt: 0,
            extra: [0; 100],
            origin: [0.0; 3],
            map: *b"MAP ",
            machst: [0; 4],
            rms: 0.0,
            nlabl: 0,
            label: [0; 800],
        }
    }

    #[inline]
    pub const fn data_offset(&self) -> usize {
        1024 + self.nsymbt as usize
    }

    #[inline]
    pub fn data_size(&self) -> usize {
        let n = (self.nx as usize) * (self.ny as usize) * (self.nz as usize);
        let bytes_per_pixel = match self.mode {
            0 | 6 => 1, // Int8, Uint8
            1 | 3 => 2, // Int16, Int16Complex
            2 | 4 => 4, // Float32, Float32Complex
            12 => 2,    // Float16
            _ => return 0,
        };
        n * bytes_per_pixel
    }

    #[inline]
    pub fn validate(&self) -> bool {
        // Fast validation with early returns
        if self.nx <= 0 || self.ny <= 0 || self.nz <= 0 {
            return false;
        }

        // Use match for faster branch prediction
        matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12)
    }

    #[inline]
    pub const fn exttyp(&self) -> i32 {
        i32::from_le_bytes([
            self.extra[8],  // Byte 105
            self.extra[9],  // Byte 106
            self.extra[10], // Byte 107
            self.extra[11], // Byte 108
        ])
    }

    #[inline]
    pub fn set_exttyp(&mut self, value: i32) {
        let bytes = value.to_le_bytes();
        self.extra[8..12].copy_from_slice(&bytes);
    }

    #[inline]
    pub fn exttyp_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.extra[8..12])
    }

    #[inline]
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
    pub const fn nversion(&self) -> i32 {
        i32::from_le_bytes([
            self.extra[12], // NVERSION at byte 109-112 (index 12-15 in extra[100])
            self.extra[13],
            self.extra[14],
            self.extra[15],
        ])
    }

    #[inline]
    pub fn set_nversion(&mut self, value: i32) {
        let bytes = value.to_le_bytes();
        self.extra[12..16].copy_from_slice(&bytes);
    }

    #[inline]
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
        // Float fields use to_bits/swap_bytes/from_bits
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

        // Swap EXTTYP (i32 at offset 8 in extra)
        let exttyp = self.exttyp().swap_bytes();
        self.set_exttyp(exttyp);

        // Swap NVERSION (i32 at offset 12 in extra)
        let nversion = self.nversion().swap_bytes();
        self.set_nversion(nversion);

        // Swap origin floats
        for val in &mut self.origin {
            *val = f32::from_bits(val.to_bits().swap_bytes());
        }
        swap_field!(nlabl);

        self.rms = f32::from_bits(self.rms.to_bits().swap_bytes());
    }
}
