#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Header {
    /// number of columns in 3D data array (fast axis)
    pub nx: i32,
    /// Number of rows in 3D data array (medium axis)
    pub ny: i32,
    /// Number of sections in 3D data array (slow axis)
    pub nz: i32,
    /// Mode value (see `Mode` enum)
    pub mode: i32,
    /// Location of first column in unit cell
    pub nxstart: i32,
    /// Location of first row in unit cell
    pub nystart: i32,
    /// Location of first section in unit cell
    pub nzstart: i32,
    /// Sampling along X axis of unit cell
    pub mx: i32,
    /// Sampling along Y axis of unit cell
    pub my: i32,
    /// Sampling along Z axis of unit cell
    pub mz: i32,
    /// CELLA: Cell dimensions (unit cell edge length) in Angstroms (Å) along X axis
    pub xlen: f32,
    /// CELLA: Cell dimensions (unit cell edge length) in Angstroms (Å) along Y axis
    pub ylen: f32,
    /// CELLA: Cell dimensions (unit cell edge length) in Angstroms (Å) along Z axis
    pub zlen: f32,
    /// CELLB: Cell angles in degrees between the crystallographic axes Y and Z axes
    pub alpha: f32,
    /// CELLB: Cell angles in degrees between the crystallographic axes X and Z axes
    pub beta: f32,
    /// CELLB: Cell angles in degrees between the crystallographic axes X and Y axes
    pub gamma: f32,
    /// 1-based index of column axis (1,2,3 for X,Y,Z)
    pub mapc: i32,
    /// 1-based index of row axis (1,2,3 for X,Y,Z)
    pub mapr: i32,
    /// 1-based index of section axis (1,2,3 for X,Y,Z)
    pub maps: i32,
    /// Minimum density value
    pub dmin: f32,
    /// Maximum density value
    pub dmax: f32,
    /// Mean density value
    pub dmean: f32,
    /// Space group number; 0 implies 2D image or image stack.
    /// For crystallography, represents the actual spacegroup.
    /// For volume stacks, conventionally ISPG = spacegroup number + 400.
    pub ispg: i32,
    /// Size of extended header (which follows main header) in bytes.
    /// May contain symmetry records or other metadata (indicated by EXTTYP).
    pub nsymbt: i32,
    /// Extra space used for anything.
    /// Bytes 8–11 hold EXTTYP, 12–15 NVERSION.
    pub extra: [u8; 100],
    /// Volume/phase origin (pixels/voxels) or origin of subvolume
    pub origin: [f32; 3],
    /// Must contain "MAP " to identify file type
    pub map: [u8; 4],
    /// Machine stamp that encodes byte order of data
    /// (little-endian: 0x44 0x44 0x00 0x00)
    pub machst: [u8; 4],
    /// RMS deviation of map from mean density
    pub rms: f32,
    /// Number of valid labels in `label` field (0–10)
    pub nlabl: i32,
    /// 10 text labels of 80 bytes each
    pub label: [u8; 800],
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
            dmin: f32::INFINITY,       // Set higher than dmax to indicate not well-determined
            dmax: f32::NEG_INFINITY,   // Set lower than dmin to indicate not well-determined
            dmean: f32::NEG_INFINITY,  // Less than both to indicate not well-determined
            ispg: 1, // P1 space group.
            nsymbt: 0,
            extra: {
                let mut arr = [0u8; 100];
                // Set NVERSION to 20141 (latest MRC2014 format version)
                // Bytes 12-15 of extra array hold NVERSION
                // 20141 = 0x4EAD, little-endian: [0xAD, 0x4E, 0x00, 0x00]
                arr[12] = 0xAD;
                arr[13] = 0x4E;
                arr[14] = 0x00;
                arr[15] = 0x00;
                arr
            },
            origin: [0.0; 3],
            map: *b"MAP ",
            machst: [0x44, 0x44, 0x00, 0x00], // Little-endian x86/AMD64.
            rms: -1.0,  // Negative indicates not well-determined
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
            0 => 1,   // 8-bit signed integer
            1 => 2,   // 16-bit signed integer
            2 => 4,   // 32-bit float
            3 => 4,   // Complex 16-bit (2 bytes real + 2 bytes imaginary)
            4 => 8,   // Complex 32-bit (4 bytes real + 4 bytes imaginary)
            6 => 2,   // 16-bit unsigned integer
            12 => 2,  // 16-bit float (IEEE-754 half)
            101 => 1, // 4-bit data packed two per byte
            _ => 0,   // unknown/unsupported
        };
        n * bytes_per_pixel
    }

    #[inline]
    /// True when dimensions are positive and mode is supported.
    pub fn validate(&self) -> bool {
        self.nx > 0
            && self.ny > 0
            && self.nz > 0
            && matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12 | 101)
            && self.map == *b"MAP "
    }

    #[inline]
    /// Reads the 4-byte EXTTYP identifier stored in `extra[8..12]`.
    pub const fn exttyp(&self) -> i32 {
        i32::from_le_bytes([self.extra[8], self.extra[9], self.extra[10], self.extra[11]])
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
    ///
    /// This value is a numeric i32 and respects the file's endianness.
    pub fn nversion(&self) -> i32 {
        let file_endian = self.detect_endian();
        let arr = [self.extra[12], self.extra[13], self.extra[14], self.extra[15]];
        match file_endian {
            crate::FileEndian::LittleEndian => i32::from_le_bytes(arr),
            crate::FileEndian::BigEndian => i32::from_be_bytes(arr),
        }
    }

    #[inline]
    /// Stores the 4-byte NVERSION number into `extra[12..16]`.
    ///
    /// This value is a numeric i32 and respects the file's endianness.
    pub fn set_nversion(&mut self, value: i32) {
        let file_endian = self.detect_endian();
        let bytes = match file_endian {
            crate::FileEndian::LittleEndian => value.to_le_bytes(),
            crate::FileEndian::BigEndian => value.to_be_bytes(),
        };
        self.extra[12..16].copy_from_slice(&bytes);
    }

    #[inline]
    /// Detect the file endianness from the MACHST machine stamp
    pub fn detect_endian(&self) -> crate::FileEndian {
        crate::FileEndian::from_machst(&self.machst)
    }

    #[inline]
    /// Check if the file is little-endian
    pub fn is_little_endian(&self) -> bool {
        self.detect_endian() == crate::FileEndian::LittleEndian
    }

    #[inline]
    /// Check if the file is big-endian
    pub fn is_big_endian(&self) -> bool {
        self.detect_endian() == crate::FileEndian::BigEndian
    }

    /// Decode header from raw bytes with correct endianness
    ///
    /// This is the ONLY safe way to read a header from raw bytes.
    /// Endianness is detected from the MACHST field and applied automatically.
    ///
    /// # Safety
    /// The input slice must be exactly 1024 bytes.
    pub fn decode_from_bytes(bytes: &[u8; 1024]) -> Self {
        use crate::FileEndian;

        // Detect endianness from MACHST (bytes 212-215)
        let machst = [bytes[212], bytes[213], bytes[214], bytes[215]];
        let file_endian = FileEndian::from_machst(&machst);

        let mut header = Self::new();

        // Helper to decode values
        let decode_i32 = |offset: usize| -> i32 {
            let arr = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
            match file_endian {
                FileEndian::LittleEndian => i32::from_le_bytes(arr),
                FileEndian::BigEndian => i32::from_be_bytes(arr),
            }
        };

        let decode_f32 = |offset: usize| -> f32 {
            let arr = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
            match file_endian {
                FileEndian::LittleEndian => f32::from_le_bytes(arr),
                FileEndian::BigEndian => f32::from_be_bytes(arr),
            }
        };

        // Read all i32 fields
        header.nx = decode_i32(0);
        header.ny = decode_i32(4);
        header.nz = decode_i32(8);
        header.mode = decode_i32(12);
        header.nxstart = decode_i32(16);
        header.nystart = decode_i32(20);
        header.nzstart = decode_i32(24);
        header.mx = decode_i32(28);
        header.my = decode_i32(32);
        header.mz = decode_i32(36);

        // Read all f32 fields
        header.xlen = decode_f32(40);
        header.ylen = decode_f32(44);
        header.zlen = decode_f32(48);
        header.alpha = decode_f32(52);
        header.beta = decode_f32(56);
        header.gamma = decode_f32(60);

        // Read axis mapping fields
        header.mapc = decode_i32(64);
        header.mapr = decode_i32(68);
        header.maps = decode_i32(72);

        // Read density statistics
        header.dmin = decode_f32(76);
        header.dmax = decode_f32(80);
        header.dmean = decode_f32(84);

        // Read space group and extended header size
        header.ispg = decode_i32(88);
        header.nsymbt = decode_i32(92);

        // Read extra bytes (bytes 96-195)
        header.extra.copy_from_slice(&bytes[96..196]);

        // Read origin coordinates
        header.origin[0] = decode_f32(196);
        header.origin[1] = decode_f32(200);
        header.origin[2] = decode_f32(204);

        // Read MAP identifier (bytes 208-211) - ASCII, no endian conversion
        header.map.copy_from_slice(&bytes[208..212]);

        // Read MACHST (bytes 212-215) - byte signature, no endian conversion
        header.machst.copy_from_slice(&bytes[212..216]);

        // Read RMS
        header.rms = decode_f32(216);

        // Read label count
        header.nlabl = decode_i32(220);

        // Read labels (bytes 224-1023) - ASCII, no endian conversion
        header.label.copy_from_slice(&bytes[224..1024]);

        header
    }

    /// Encode header to raw bytes with correct endianness
    ///
    /// This is the ONLY safe way to write a header to raw bytes.
    /// Endianness is determined from the MACHST field and applied automatically.
    ///
    /// # Safety
    /// The output slice must be exactly 1024 bytes.
    pub fn encode_to_bytes(&self, out: &mut [u8; 1024]) {
        use crate::FileEndian;

        let file_endian = self.detect_endian();

        // Helper macros to encode values
        macro_rules! encode_i32 {
            ($offset:expr, $value:expr) => {
                let bytes = match file_endian {
                    FileEndian::LittleEndian => $value.to_le_bytes(),
                    FileEndian::BigEndian => $value.to_be_bytes(),
                };
                out[$offset..$offset + 4].copy_from_slice(&bytes);
            };
        }

        macro_rules! encode_f32 {
            ($offset:expr, $value:expr) => {
                let bytes = match file_endian {
                    FileEndian::LittleEndian => $value.to_le_bytes(),
                    FileEndian::BigEndian => $value.to_be_bytes(),
                };
                out[$offset..$offset + 4].copy_from_slice(&bytes);
            };
        }

        // Write all i32 fields
        encode_i32!(0, self.nx);
        encode_i32!(4, self.ny);
        encode_i32!(8, self.nz);
        encode_i32!(12, self.mode);
        encode_i32!(16, self.nxstart);
        encode_i32!(20, self.nystart);
        encode_i32!(24, self.nzstart);
        encode_i32!(28, self.mx);
        encode_i32!(32, self.my);
        encode_i32!(36, self.mz);

        // Write all f32 fields
        encode_f32!(40, self.xlen);
        encode_f32!(44, self.ylen);
        encode_f32!(48, self.zlen);
        encode_f32!(52, self.alpha);
        encode_f32!(56, self.beta);
        encode_f32!(60, self.gamma);

        // Write axis mapping fields
        encode_i32!(64, self.mapc);
        encode_i32!(68, self.mapr);
        encode_i32!(72, self.maps);

        // Write density statistics
        encode_f32!(76, self.dmin);
        encode_f32!(80, self.dmax);
        encode_f32!(84, self.dmean);

        // Write space group and extended header size
        encode_i32!(88, self.ispg);
        encode_i32!(92, self.nsymbt);

        // Write extra bytes (bytes 96-195)
        out[96..196].copy_from_slice(&self.extra);

        // Write origin coordinates
        encode_f32!(196, self.origin[0]);
        encode_f32!(200, self.origin[1]);
        encode_f32!(204, self.origin[2]);

        // Write MAP identifier (bytes 208-211) - ASCII, no endian conversion
        out[208..212].copy_from_slice(&self.map);

        // Write MACHST (bytes 212-215) - byte signature, no endian conversion
        out[212..216].copy_from_slice(&self.machst);

        // Write RMS
        encode_f32!(216, self.rms);

        // Write label count
        encode_i32!(220, self.nlabl);

        // Write labels (bytes 224-1023) - ASCII, no endian conversion
        out[224..1024].copy_from_slice(&self.label);
    }
}
