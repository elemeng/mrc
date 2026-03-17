use crate::Mode;

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
    ///
    /// # Endianness
    /// Per crate policy, new MRC files are always written in little-endian format.
    /// This constructor sets `machst` to little-endian by default. The `extra[12..16]`
    /// (NVERSION) field is uninitialized and should be set via `set_nversion()` when needed.
    ///
    /// # Example
    /// ```ignore
    /// let mut header = Header::new();
    /// header.set_nversion(20141);
    /// ```
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
            mapc: 1,                  // Column → X
            mapr: 2,                  // Row    → Y
            maps: 3,                  // Section→ Z
            dmin: f32::INFINITY,      // Set higher than dmax to indicate not well-determined
            dmax: f32::NEG_INFINITY,  // Set lower than dmin to indicate not well-determined
            dmean: f32::NEG_INFINITY, // Less than both to indicate not well-determined
            ispg: 1,                  // P1 space group.
            nsymbt: 0,
            extra: [0u8; 100], // NVERSION not set (no premature encoding)
            origin: [0.0; 3],
            map: *b"MAP ",
            machst: [0x44, 0x44, 0x00, 0x00], // Little-endian (crate policy for new files)
            rms: -1.0,                        // Negative indicates not well-determined
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
        match Mode::from_i32(self.mode) {
            Some(mode) => {
                let byte_size = mode.byte_size();
                match mode {
                    Mode::Packed4Bit => n.div_ceil(2), // two voxels per byte
                    _ => n * byte_size,
                }
            }
            None => 0, // unknown/unsupported
        }
    }

    #[inline]
    /// True when dimensions are positive and mode is supported.
    pub fn validate(&self) -> bool {
        self.nx > 0
            && self.ny > 0
            && self.nz > 0
            && matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12 | 101)
            && self.validate_map()
            // Validate ISPG: 0 (2D/stack), 1-230 (crystallographic), or 400-630 (volume stacks)
            && (self.ispg == 0 || (self.ispg >= 1 && self.ispg <= 230) || (self.ispg >= 400 && self.ispg <= 630))
            // Validate axis mapping: MAPC, MAPR, MAPS must be a permutation of (1, 2, 3)
            && matches!(self.mapc, 1..=3)
            && matches!(self.mapr, 1..=3)
            && matches!(self.maps, 1..=3)
            && self.mapc != self.mapr
            && self.mapc != self.maps
            && self.mapr != self.maps
            // Validate nsymbt is non-negative
            && self.nsymbt >= 0
            // Validate nlabl is between 0 and 10
            && self.nlabl >= 0 && self.nlabl <= 10
    }

    #[inline]
    /// Validate the MAP field, allowing for legacy variants.
    ///
    /// Standard MRC2014 requires "MAP ", but some legacy files may use:
    /// - "MAP\0" (null-terminated)
    /// - "MAPI" (older format)
    /// - All zeros (uninitialized)
    fn validate_map(&self) -> bool {
        // Standard MRC2014 format
        if self.map == *b"MAP " {
            return true;
        }
        // Accept legacy variants: "MAP\0" or "MAPI"
        if &self.map[..3] == b"MAP"
            && (self.map[3] == b' ' || self.map[3] == 0 || self.map[3] == b'I')
        {
            return true;
        }
        // Accept all zeros (uninitialized, common in some generated files)
        if self.map == [0; 4] {
            return true;
        }
        false
    }

    #[inline]
    /// Reads the 4-byte EXTTYP identifier stored in `extra[8..12]`.
    ///
    /// EXTTYP is a 4-byte ASCII string indicating the type of extended header.
    /// Common values: "CCP4", "MRCO", "SERI", "AGAR", "FEI1", "FEI2", "HDF5".
    pub fn exttyp(&self) -> [u8; 4] {
        [self.extra[8], self.extra[9], self.extra[10], self.extra[11]]
    }

    #[inline]
    /// Stores the 4-byte EXTTYP identifier into `extra[8..12]`.
    ///
    /// EXTTYP is a 4-byte ASCII string indicating the type of extended header.
    pub fn set_exttyp(&mut self, value: [u8; 4]) {
        self.extra[8..12].copy_from_slice(&value);
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
        self.extra[8..12].copy_from_slice(bytes);
        Ok(())
    }

    #[inline]
    /// Reads the 4-byte NVERSION number stored in `extra[12..16]`.
    ///
    /// This value is a numeric i32 and respects the file's endianness.
    pub fn nversion(&self) -> i32 {
        use crate::engine::codec::EndianCodec;
        let file_endian = self.detect_endian();
        i32::decode(&self.extra[12..16], 0, file_endian)
    }

    #[inline]
    /// Stores the 4-byte NVERSION number into `extra[12..16]`.
    ///
    /// This value is a numeric i32 and respects the file's endianness.
    pub fn set_nversion(&mut self, value: i32) {
        use crate::engine::codec::EndianCodec;
        let file_endian = self.detect_endian();
        value.encode(&mut self.extra[12..16], 0, file_endian);
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

    #[inline]
    /// Set the file endianness for this header
    ///
    /// This sets the MACHST machine stamp to the appropriate value for the
    /// specified endianness. This is primarily used internally when reading
    /// existing files to preserve their endianness.
    ///
    /// # Note
    /// Per crate policy, new MRC files are always written in little-endian format.
    /// This method is not intended for creating big-endian files from scratch.
    pub fn set_file_endian(&mut self, endian: crate::FileEndian) {
        self.machst = endian.to_machst();
    }

    /// Decode header from raw bytes with correct endianness
    ///
    /// This is the ONLY safe way to read a header from raw bytes.
    /// Endianness is detected from the MACHST field and applied automatically.
    ///
    /// # Safety
    /// The input slice must be exactly 1024 bytes.
    pub fn decode_from_bytes(bytes: &[u8; 1024]) -> Self {
        use crate::engine::codec::EndianCodec;
        use crate::engine::endian::FileEndian;

        // Detect endianness from MACHST (bytes 212-215)
        let machst = [bytes[212], bytes[213], bytes[214], bytes[215]];
        let file_endian = FileEndian::from_machst(&machst);

        let mut header = Self::new();

        // Read all i32 fields
        header.nx = i32::decode(bytes, 0, file_endian);
        header.ny = i32::decode(bytes, 4, file_endian);
        header.nz = i32::decode(bytes, 8, file_endian);
        header.mode = i32::decode(bytes, 12, file_endian);
        header.nxstart = i32::decode(bytes, 16, file_endian);
        header.nystart = i32::decode(bytes, 20, file_endian);
        header.nzstart = i32::decode(bytes, 24, file_endian);
        header.mx = i32::decode(bytes, 28, file_endian);
        header.my = i32::decode(bytes, 32, file_endian);
        header.mz = i32::decode(bytes, 36, file_endian);

        // Read all f32 fields
        header.xlen = f32::decode(bytes, 40, file_endian);
        header.ylen = f32::decode(bytes, 44, file_endian);
        header.zlen = f32::decode(bytes, 48, file_endian);
        header.alpha = f32::decode(bytes, 52, file_endian);
        header.beta = f32::decode(bytes, 56, file_endian);
        header.gamma = f32::decode(bytes, 60, file_endian);

        // Read axis mapping fields
        header.mapc = i32::decode(bytes, 64, file_endian);
        header.mapr = i32::decode(bytes, 68, file_endian);
        header.maps = i32::decode(bytes, 72, file_endian);

        // Read density statistics
        header.dmin = f32::decode(bytes, 76, file_endian);
        header.dmax = f32::decode(bytes, 80, file_endian);
        header.dmean = f32::decode(bytes, 84, file_endian);

        // Read space group and extended header size
        header.ispg = i32::decode(bytes, 88, file_endian);
        header.nsymbt = i32::decode(bytes, 92, file_endian);

        // Read extra bytes (bytes 96-195)
        header.extra.copy_from_slice(&bytes[96..196]);

        // Read origin coordinates
        header.origin[0] = f32::decode(bytes, 196, file_endian);
        header.origin[1] = f32::decode(bytes, 200, file_endian);
        header.origin[2] = f32::decode(bytes, 204, file_endian);

        // Read MAP identifier (bytes 208-211) - ASCII, no endian conversion
        header.map.copy_from_slice(&bytes[208..212]);

        // Read MACHST (bytes 212-215) - byte signature, no endian conversion
        header.machst.copy_from_slice(&bytes[212..216]);

        // Read RMS
        header.rms = f32::decode(bytes, 216, file_endian);

        // Read label count
        header.nlabl = i32::decode(bytes, 220, file_endian);

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
        use crate::engine::codec::EndianCodec;

        let file_endian = self.detect_endian();

        // Write all i32 fields
        self.nx.encode(out, 0, file_endian);
        self.ny.encode(out, 4, file_endian);
        self.nz.encode(out, 8, file_endian);
        self.mode.encode(out, 12, file_endian);
        self.nxstart.encode(out, 16, file_endian);
        self.nystart.encode(out, 20, file_endian);
        self.nzstart.encode(out, 24, file_endian);
        self.mx.encode(out, 28, file_endian);
        self.my.encode(out, 32, file_endian);
        self.mz.encode(out, 36, file_endian);

        // Write all f32 fields
        self.xlen.encode(out, 40, file_endian);
        self.ylen.encode(out, 44, file_endian);
        self.zlen.encode(out, 48, file_endian);
        self.alpha.encode(out, 52, file_endian);
        self.beta.encode(out, 56, file_endian);
        self.gamma.encode(out, 60, file_endian);

        // Write axis mapping fields
        self.mapc.encode(out, 64, file_endian);
        self.mapr.encode(out, 68, file_endian);
        self.maps.encode(out, 72, file_endian);

        // Write density statistics
        self.dmin.encode(out, 76, file_endian);
        self.dmax.encode(out, 80, file_endian);
        self.dmean.encode(out, 84, file_endian);

        // Write space group and extended header size
        self.ispg.encode(out, 88, file_endian);
        self.nsymbt.encode(out, 92, file_endian);

        // Write extra bytes (bytes 96-195)
        out[96..196].copy_from_slice(&self.extra);

        // Write origin coordinates
        self.origin[0].encode(out, 196, file_endian);
        self.origin[1].encode(out, 200, file_endian);
        self.origin[2].encode(out, 204, file_endian);

        // Write MAP identifier (bytes 208-211) - ASCII, no endian conversion
        out[208..212].copy_from_slice(&self.map);

        // Write MACHST (bytes 212-215) - byte signature, no endian conversion
        out[212..216].copy_from_slice(&self.machst);

        // Write RMS
        self.rms.encode(out, 216, file_endian);

        // Write label count
        self.nlabl.encode(out, 220, file_endian);

        // Write labels (bytes 224-1023) - ASCII, no endian conversion
        out[224..1024].copy_from_slice(&self.label);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExtHeader<'a> {
    bytes: &'a [u8],
}

impl<'a> ExtHeader<'a> {
    #[inline]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Debug)]
pub struct ExtHeaderMut<'a> {
    bytes: &'a mut [u8],
}

impl<'a> ExtHeaderMut<'a> {
    #[inline]
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.bytes
    }
}
