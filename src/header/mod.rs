//! MRC-2014 header structure and builder.
//!
//! The [`Header`] struct mirrors the 1024-byte fixed header defined by the
//! MRC-2014 specification. Every field is a typed public member — dimensions,
//! cell parameters, axis mapping, density statistics, text labels, and more.
//!
//! The `Header` provides encode/decode methods for raw bytes, validation
//! helpers at three levels (basic, detailed, permissive), and convenience
//! accessors for common metadata (voxel size, cell parameters, volume type,
//! labels, FEI extended header info).
//!
//! Use [`HeaderBuilder`] to construct new headers with a fluent API that
//! validates on build.
//!
//! # Example — decode/encode round-trip
//!
//! ```
//! use mrc::Header;
//!
//! let mut raw = [0u8; 1024];
//! // Standard MRC-2014 markers
//! raw[208..212].copy_from_slice(b"MAP ");
//! // Little-endian MACHST
//! raw[212..216].copy_from_slice(&[0x44, 0x44, 0x00, 0x00]);
//! // Dimensions: 64 x 64 x 1
//! raw[0..4].copy_from_slice(&(64i32).to_le_bytes());
//! raw[4..8].copy_from_slice(&(64i32).to_le_bytes());
//! raw[8..12].copy_from_slice(&(1i32).to_le_bytes());
//! // Mode 2 (Float32)
//! raw[12..16].copy_from_slice(&(2i32).to_le_bytes());
//!
//! let header = Header::decode_from_bytes(&raw);
//! assert_eq!(header.nx, 64);
//! assert_eq!(header.ny, 64);
//! assert_eq!(header.nz, 1);
//! assert_eq!(header.mode, 2);
//!
//! let mut encoded = [0u8; 1024];
//! header.encode_to_bytes(&mut encoded);
//! assert_eq!(raw, encoded);
//! ```

pub mod agar;
pub mod ccp4;
pub mod fei;
pub mod mrco;
pub mod seri;

pub use agar::{AGAR_RECORD_SIZE, AgarRecord, parse_agar_records};
pub use ccp4::{CCP4_RECORD_SIZE, Ccp4Record, parse_ccp4_records};
pub use fei::{
    FEI1_RECORD_SIZE, FEI2_RECORD_SIZE, Fei1Metadata, Fei2Metadata, parse_fei1_records,
    parse_fei2_records,
};
pub use mrco::{MRCO_RECORD_SIZE, MrcoRecord, parse_mrco_records};
pub use seri::{SERI_RECORD_SIZE, SeriRecord, parse_seri_records};

use crate::Mode;

// Header field offsets (MRC2014 format)
const OFFSET_NX: usize = 0;
const OFFSET_NY: usize = 4;
const OFFSET_NZ: usize = 8;
const OFFSET_MODE: usize = 12;
const OFFSET_NXSTART: usize = 16;
const OFFSET_NYSTART: usize = 20;
const OFFSET_NZSTART: usize = 24;
const OFFSET_MX: usize = 28;
const OFFSET_MY: usize = 32;
const OFFSET_MZ: usize = 36;
const OFFSET_XLEN: usize = 40;
const OFFSET_YLEN: usize = 44;
const OFFSET_ZLEN: usize = 48;
const OFFSET_ALPHA: usize = 52;
const OFFSET_BETA: usize = 56;
const OFFSET_GAMMA: usize = 60;
const OFFSET_MAPC: usize = 64;
const OFFSET_MAPR: usize = 68;
const OFFSET_MAPS: usize = 72;
const OFFSET_DMIN: usize = 76;
const OFFSET_DMAX: usize = 80;
const OFFSET_DMEAN: usize = 84;
const OFFSET_ISPG: usize = 88;
const OFFSET_NSYMBT: usize = 92;
const OFFSET_EXTRA: usize = 96;
const OFFSET_EXTTYP: usize = 104; // extra[8..12]
const OFFSET_NVERSION: usize = 108; // extra[12..16]
const OFFSET_ORIGIN: usize = 196;
const OFFSET_MAP: usize = 208;
const OFFSET_MACHST: usize = 212;
const OFFSET_RMS: usize = 216;
const OFFSET_NLABL: usize = 220;
const OFFSET_LABEL: usize = 224;

/// Default `extra` bytes with NVERSION=20141 encoded in little-endian.
const DEFAULT_EXTRA: [u8; 100] = {
    let mut e = [0u8; 100];
    // NVERSION = 20141 (latest MRC2014 update), stored little-endian in extra[12..16]
    e[12] = 0xAD;
    e[13] = 0x4E;
    e[14] = 0x00;
    e[15] = 0x00;
    e
};
// Official MRC2014 specification: https://www.ccpem.ac.uk/mrc-format/mrc2014/
#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Header {
    /// Number of columns in 3D data array (fast axis)
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
    /// CELLA: Cell dimensions (unit cell edge length) in angstroms (Å) along X axis
    pub xlen: f32,
    /// CELLA: Cell dimensions (unit cell edge length) in angstroms (Å) along Y axis
    pub ylen: f32,
    /// CELLA: Cell dimensions (unit cell edge length) in angstroms (Å) along Z axis
    pub zlen: f32,
    /// CELLB: Cell angles in degrees between the crystallographic axes Y and Z axes
    pub alpha: f32,
    /// CELLB: Cell angles in degrees between the crystallographic axes X and Z axes
    pub beta: f32,
    /// CELLB: Cell angles in degrees between the crystallographic axes X and Y axes
    pub gamma: f32,
    /// One-based index of column axis (1, 2, 3 for X, Y, Z)
    pub mapc: i32,
    /// One-based index of row axis (1, 2, 3 for X, Y, Z)
    pub mapr: i32,
    /// One-based index of section axis (1, 2, 3 for X, Y, Z)
    pub maps: i32,
    /// Minimum density value
    pub dmin: f32,
    /// Maximum density value
    pub dmax: f32,
    /// Mean density value
    pub dmean: f32,
    /// Space group number; 0 implies 2D image or image stack.
    ///
    /// For crystallography, represents the actual space group.
    /// For volume stacks, conventionally ISPG = space group number + 400.
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
    /// Machine stamp that encodes byte order of data.
    ///
    /// Little-endian files use `0x44 0x44 0x00 0x00`.
    pub machst: [u8; 4],
    /// RMS deviation of map from mean density
    pub rms: f32,
    /// Number of valid labels in `label` field (0–10)
    pub nlabl: i32,
    /// Ten text labels of 80 bytes each.
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
    /// This constructor sets `machst` to little-endian by default and initialises
    /// `nversion` to `20141` (latest MRC2014 update).
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
            mapc: 1,     // Column → X
            mapr: 2,     // Row    → Y
            maps: 3,     // Section→ Z
            dmin: 0.0,   // Set higher than dmax to indicate not well-determined
            dmax: -1.0,  // Set lower than dmin to indicate not well-determined
            dmean: -2.0, // Less than both to indicate not well-determined
            ispg: 1,     // P1 space group.
            nsymbt: 0,
            extra: DEFAULT_EXTRA,
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
    ///
    /// Returns `1024` when `nsymbt` is negative (to avoid integer wrap-around
    /// on malformed headers).
    ///
    /// ```
    /// use mrc::Header;
    /// let h = Header::new();
    /// assert_eq!(h.data_offset(), 1024);
    /// ```
    pub const fn data_offset(&self) -> usize {
        if self.nsymbt < 0 {
            1024
        } else {
            1024 + self.nsymbt as usize
        }
    }

    #[inline]
    /// Size, in bytes, of the voxel data block.
    ///
    /// Returns `None` if the dimensions are so large that the calculation
    /// overflows `usize`.
    ///
    /// ```
    /// use mrc::Header;
    /// let mut h = Header::new();
    /// h.nx = 64; h.ny = 64; h.nz = 32;
    /// h.mode = 2; // Float32 → 4 bytes per voxel
    /// assert_eq!(h.data_size(), Some(64 * 64 * 32 * 4));
    /// ```
    pub fn data_size(&self) -> Option<usize> {
        let nx = self.nx.max(0) as usize;
        let ny = self.ny.max(0) as usize;
        let nz = self.nz.max(0) as usize;
        match Mode::from_i32(self.mode) {
            Some(mode) => {
                match mode {
                    // For Packed4Bit, each row is padded to a whole byte boundary:
                    // row_bytes = nx.div_ceil(2), total = ny * row_bytes * nz
                    Mode::Packed4Bit => {
                        let row_bytes = nx.div_ceil(2);
                        ny.checked_mul(row_bytes)?.checked_mul(nz)
                    }
                    _ => nx
                        .checked_mul(ny)?
                        .checked_mul(nz)?
                        .checked_mul(mode.byte_size()),
                }
            }
            None => None, // unknown/unsupported mode
        }
    }

    #[inline]
    /// True when dimensions are positive and mode is supported.
    ///
    /// ```
    /// use mrc::Header;
    /// let h = Header::new();
    /// // Default header has zero dimensions → invalid
    /// assert!(!h.validate());
    /// ```
    pub fn validate(&self) -> bool {
        self.validate_detailed().is_ok()
    }

    #[inline]
    /// Detailed header validation returning specific error information.
    ///
    /// ```
    /// use mrc::Header;
    /// let h = Header::new();
    /// match h.validate_detailed() {
    ///     Err(e) => assert!(e.to_string().contains("dimensions")),
    ///     Ok(()) => unreachable!(),
    /// }
    /// ```
    pub fn validate_detailed(&self) -> Result<(), crate::HeaderValidationError> {
        use crate::HeaderValidationError;

        if self.nx <= 0 || self.ny <= 0 || self.nz <= 0 {
            return Err(HeaderValidationError::InvalidDimensions {
                nx: self.nx,
                ny: self.ny,
                nz: self.nz,
            });
        }

        if Mode::from_i32(self.mode).is_none() {
            return Err(HeaderValidationError::UnsupportedMode(self.mode));
        }

        if !self.validate_map() {
            return Err(HeaderValidationError::InvalidMap(self.map));
        }

        if !(self.ispg == 0
            || (self.ispg >= 1 && self.ispg <= 230)
            || (self.ispg >= 400 && self.ispg <= 630))
        {
            return Err(HeaderValidationError::InvalidIspg(self.ispg));
        }

        if !(matches!(self.mapc, 1..=3)
            && matches!(self.mapr, 1..=3)
            && matches!(self.maps, 1..=3)
            && self.mapc != self.mapr
            && self.mapc != self.maps
            && self.mapr != self.maps)
        {
            return Err(HeaderValidationError::InvalidAxisMapping {
                mapc: self.mapc,
                mapr: self.mapr,
                maps: self.maps,
            });
        }

        if self.nsymbt < 0 {
            return Err(HeaderValidationError::InvalidNsymbt(self.nsymbt));
        }

        if self.nlabl < 0 || self.nlabl > 10 {
            return Err(HeaderValidationError::InvalidNlabl(self.nlabl));
        }

        // Label sequence validation: nlabl must match actual non-empty labels,
        // and no empty labels may appear between filled ones.
        let actual_labels = self.count_non_empty_labels();
        if actual_labels != self.nlabl as usize {
            return Err(HeaderValidationError::LabelCountMismatch {
                nlabl: self.nlabl,
                actual: actual_labels as i32,
            });
        }
        for i in 0..self.nlabl as usize {
            if self.label_is_empty(i) {
                return Err(HeaderValidationError::EmptyLabelBeforeFilled { index: i as i32 });
            }
        }

        let nversion = self.nversion();
        if nversion != 0 && nversion != 20140 && nversion != 20141 {
            return Err(HeaderValidationError::InvalidNversion(nversion));
        }

        if self.mx <= 0 || self.my <= 0 || self.mz <= 0 {
            return Err(HeaderValidationError::InvalidSampling {
                mx: self.mx,
                my: self.my,
                mz: self.mz,
            });
        }

        if self.ispg >= 400 && self.ispg <= 630 && self.mz != 0 && self.nz % self.mz != 0 {
            return Err(HeaderValidationError::InvalidVolumeStack {
                nz: self.nz,
                mz: self.mz,
                ispg: self.ispg,
            });
        }

        Ok(())
    }

    /// Permissive validation that returns warnings instead of hard errors
    /// for most non-critical issues.
    ///
    /// Only **fatal** problems (dimensions ≤ 0 or completely unsupported mode)
    /// produce an `Err`. Everything else is collected as a human-readable
    /// warning string.
    pub fn validate_permissive(&self) -> Result<Vec<String>, crate::HeaderValidationError> {
        use crate::HeaderValidationError;
        let mut warnings = Vec::new();

        if self.nx <= 0 || self.ny <= 0 || self.nz <= 0 {
            return Err(HeaderValidationError::InvalidDimensions {
                nx: self.nx,
                ny: self.ny,
                nz: self.nz,
            });
        }

        if Mode::from_i32(self.mode).is_none() {
            return Err(HeaderValidationError::UnsupportedMode(self.mode));
        }

        if !self.validate_map() {
            warnings.push(format!(
                "MAP field is non-standard: {:?}",
                String::from_utf8_lossy(&self.map)
            ));
        }

        if !(self.ispg == 0
            || (self.ispg >= 1 && self.ispg <= 230)
            || (self.ispg >= 400 && self.ispg <= 630))
        {
            warnings.push(format!(
                "ISPG {} is outside the standard ranges (0, 1-230, 400-630)",
                self.ispg
            ));
        }

        if !(matches!(self.mapc, 1..=3)
            && matches!(self.mapr, 1..=3)
            && matches!(self.maps, 1..=3)
            && self.mapc != self.mapr
            && self.mapc != self.maps
            && self.mapr != self.maps)
        {
            warnings.push(format!(
                "Axis mapping ({}, {}, {}) is not a permutation of (1, 2, 3)",
                self.mapc, self.mapr, self.maps
            ));
        }

        if self.nsymbt < 0 {
            warnings.push(format!("NSYMBT is negative ({})", self.nsymbt));
        }

        if self.nlabl < 0 || self.nlabl > 10 {
            warnings.push(format!("NLABL {} is outside 0-10", self.nlabl));
        }

        let nversion = self.nversion();
        if nversion != 20140 && nversion != 20141 {
            warnings.push(format!("NVERSION {} is not 20140 or 20141", nversion));
        }

        if self.mx <= 0 || self.my <= 0 || self.mz <= 0 {
            warnings.push(format!(
                "Sampling (mx={}, my={}, mz={}) is not all positive",
                self.mx, self.my, self.mz
            ));
        }

        if self.ispg >= 400 && self.ispg <= 630 && self.mz != 0 && self.nz % self.mz != 0 {
            warnings.push(format!(
                "Volume stack: nz ({}) is not divisible by mz ({}) for ispg={}",
                self.nz, self.mz, self.ispg
            ));
        }

        Ok(warnings)
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
    ///
    /// ```
    /// use mrc::Header;
    /// let mut h = Header::new();
    /// h.set_exttyp(*b"CCP4");
    /// assert_eq!(h.exttyp(), *b"CCP4");
    /// ```
    pub fn exttyp(&self) -> [u8; 4] {
        [
            self.extra[OFFSET_EXTTYP - OFFSET_EXTRA],
            self.extra[OFFSET_EXTTYP - OFFSET_EXTRA + 1],
            self.extra[OFFSET_EXTTYP - OFFSET_EXTRA + 2],
            self.extra[OFFSET_EXTTYP - OFFSET_EXTRA + 3],
        ]
    }

    #[inline]
    /// Stores the 4-byte EXTTYP identifier into `extra[8..12]`.
    ///
    /// EXTTYP is a 4-byte ASCII string indicating the type of extended header.
    pub fn set_exttyp(&mut self, value: [u8; 4]) {
        let start = OFFSET_EXTTYP - OFFSET_EXTRA;
        self.extra[start..start + 4].copy_from_slice(&value);
    }

    #[inline]
    /// Interprets EXTTYP as an ASCII string.
    pub fn exttyp_str(&self) -> Result<&str, core::str::Utf8Error> {
        let start = OFFSET_EXTTYP - OFFSET_EXTRA;
        core::str::from_utf8(&self.extra[start..start + 4])
    }

    #[inline]
    /// Sets EXTTYP from a 4-character ASCII string.
    pub fn set_exttyp_str(&mut self, value: &str) -> Result<(), &'static str> {
        if value.len() != 4 {
            return Err("EXTTYP must be exactly 4 characters");
        }
        let bytes = value.as_bytes();
        let start = OFFSET_EXTTYP - OFFSET_EXTRA;
        self.extra[start..start + 4].copy_from_slice(bytes);
        Ok(())
    }

    #[inline]
    /// Reads the 4-byte NVERSION number stored in `extra[12..16]`.
    ///
    /// This value is a numeric i32 and respects the file's endianness.
    ///
    /// ```
    /// use mrc::Header;
    /// let h = Header::new();
    /// assert_eq!(h.nversion(), 20141);
    /// ```
    pub fn nversion(&self) -> i32 {
        use crate::engine::codec::EndianCodec;
        let file_endian = self.detect_endian();
        let start = OFFSET_NVERSION - OFFSET_EXTRA;
        i32::decode(&self.extra[start..start + 4], 0, file_endian)
    }

    #[inline]
    /// Stores the 4-byte NVERSION number into `extra[12..16]`.
    ///
    /// This value is a numeric i32 and respects the file's endianness.
    pub fn set_nversion(&mut self, value: i32) {
        use crate::engine::codec::EndianCodec;
        let file_endian = self.detect_endian();
        let start = OFFSET_NVERSION - OFFSET_EXTRA;
        value.encode(&mut self.extra[start..start + 4], 0, file_endian);
    }

    /// Get the list of non-empty text labels.
    ///
    /// Returns up to `nlabl` labels, each trimmed of trailing whitespace.
    ///
    /// ```
    /// use mrc::Header;
    /// let mut h = Header::new();
    /// h.add_label("my sample");
    /// h.add_label("defocus series");
    /// let labels = h.get_labels();
    /// assert_eq!(labels, vec!["my sample", "defocus series"]);
    /// ```
    pub fn get_labels(&self) -> Vec<String> {
        let count = self.nlabl.clamp(0, 10) as usize;
        let mut labels = Vec::with_capacity(count);
        for i in 0..count {
            let start = i * 80;
            let bytes = &self.label[start..start + 80];
            let text = String::from_utf8_lossy(bytes);
            labels.push(text.trim_end().to_string());
        }
        labels
    }

    /// Check whether the i-th label is empty (all whitespace / zeros).
    fn label_is_empty(&self, index: usize) -> bool {
        let start = index * 80;
        self.label[start..start + 80]
            .iter()
            .all(|&b| b == 0 || b == b' ')
    }

    /// Count how many of the 10 label slots contain non-empty text.
    fn count_non_empty_labels(&self) -> usize {
        (0..10).filter(|&i| !self.label_is_empty(i)).count()
    }

    /// Add a text label to the header.
    ///
    /// Labels are truncated to 80 bytes and non-printable ASCII characters
    /// (outside 0x20–0x7E) are replaced with spaces. If 10 labels are already
    /// stored, the oldest label is dropped (FIFO).
    pub fn add_label(&mut self, text: &str) {
        // Filter to printable ASCII and truncate to 80 bytes
        let filtered: String = text
            .chars()
            .map(|c| {
                if c.is_ascii_graphic() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .take(80)
            .collect();
        let bytes = filtered.as_bytes();
        let len = bytes.len();

        let count = self.count_non_empty_labels();
        if count < 10 {
            // Find the first empty slot
            let slot = (0..10).find(|&i| self.label_is_empty(i)).unwrap_or(count);
            let start = slot * 80;
            self.label[start..start + 80].fill(b' ');
            self.label[start..start + len].copy_from_slice(bytes);
        } else {
            // Shift existing labels up (FIFO) and store in slot 9.
            // Drop slot 0 (oldest), shift slots 1..9 to 0..8.
            self.label.copy_within(80..800, 0);
            let start = 9 * 80;
            self.label[start..start + 80].fill(b' ');
            self.label[start..start + len].copy_from_slice(bytes);
        }
        self.nlabl = self.count_non_empty_labels() as i32;
    }

    #[inline]
    /// Detect the file endianness from the MACHST machine stamp
    ///
    /// ```
    /// use mrc::{Header, FileEndian};
    /// let h = Header::new();
    /// assert_eq!(h.detect_endian(), FileEndian::LittleEndian);
    /// ```
    pub fn detect_endian(&self) -> crate::FileEndian {
        crate::FileEndian::from_machst(&self.machst)
    }

    #[inline]
    /// Set the file endianness for this header
    ///
    /// This sets the MACHST machine stamp to the appropriate value for the
    /// specified endianness and re-encodes NVERSION so that it remains valid.
    ///
    /// # Note
    /// Per crate policy, new MRC files are always written in little-endian format.
    /// This method is not intended for creating big-endian files from scratch.
    ///
    /// ```
    /// use mrc::{Header, FileEndian};
    /// let mut h = Header::new();
    /// h.set_file_endian(FileEndian::BigEndian);
    /// assert_eq!(h.detect_endian(), FileEndian::BigEndian);
    /// ```
    pub fn set_file_endian(&mut self, endian: crate::FileEndian) {
        // Preserve the current nversion value before swapping endianness,
        // then re-encode it in the new byte order.
        let current_nversion = self.nversion();
        self.machst = endian.to_machst();
        self.set_nversion(current_nversion);
    }

    // -------------------------------------------------------------------------
    // Volume type introspection (following Python mrcfile conventions)
    // -------------------------------------------------------------------------

    /// Returns `true` if this is a single 2D image (`nz == 1`).
    ///
    /// ```
    /// use mrc::Header;
    /// let mut h = Header::new();
    /// h.nz = 1;
    /// assert!(h.is_single_image());
    /// h.nz = 10;
    /// assert!(!h.is_single_image());
    /// ```
    pub fn is_single_image(&self) -> bool {
        self.nz == 1
    }

    /// Returns `true` if this is an image stack (`ispg == 0`).
    ///
    /// ```
    /// use mrc::Header;
    /// let mut h = Header::new();
    /// h.ispg = 0;
    /// assert!(h.is_image_stack());
    /// ```
    pub fn is_image_stack(&self) -> bool {
        self.ispg == 0
    }

    /// Returns `true` if this is a single 3D volume (`ispg != 0` and not a
    /// volume stack).
    pub fn is_volume(&self) -> bool {
        !self.is_image_stack() && !self.is_volume_stack()
    }

    /// Returns `true` if this is a volume stack (`ispg` in 401–630).
    pub fn is_volume_stack(&self) -> bool {
        (400..=630).contains(&self.ispg)
    }

    /// Configure the header as an image stack.
    ///
    /// Sets `ispg = 0` and `mz = 1`.
    pub fn set_image_stack(&mut self) {
        self.ispg = 0;
        self.mz = 1;
    }

    /// Configure the header as a single volume.
    ///
    /// Sets `ispg = 1` and `mz = nz`.
    pub fn set_volume(&mut self) {
        self.ispg = 1;
        self.mz = self.nz;
    }

    /// Configure the header as a volume stack.
    ///
    /// Sets `ispg = 401` and `mz` to the given sub-volume size.
    /// `nz` must be divisible by `mz` for the header to validate.
    pub fn set_volume_stack(&mut self, mz: i32) {
        self.ispg = 401;
        self.mz = mz;
    }

    // -------------------------------------------------------------------------
    // Computed convenience properties
    // -------------------------------------------------------------------------

    /// Voxel size in Ångströms per pixel, computed as `cella / mxyz`.
    ///
    /// Returns `[xlen / mx, ylen / my, zlen / mz]`.
    /// If any of `mx`, `my`, `mz` is zero, that component returns `0.0`.
    pub fn voxel_size(&self) -> [f32; 3] {
        [
            if self.mx == 0 {
                0.0
            } else {
                self.xlen / self.mx as f32
            },
            if self.my == 0 {
                0.0
            } else {
                self.ylen / self.my as f32
            },
            if self.mz == 0 {
                0.0
            } else {
                self.zlen / self.mz as f32
            },
        ]
    }

    /// Starting grid point / origin offset.
    ///
    /// Returns `[nxstart, nystart, nzstart]`.
    pub fn nstart(&self) -> [i32; 3] {
        [self.nxstart, self.nystart, self.nzstart]
    }

    /// Cell dimensions (unit cell edge lengths) in ångströms.
    ///
    /// Returns `[xlen, ylen, zlen]`.
    pub fn cell_lengths(&self) -> [f32; 3] {
        [self.xlen, self.ylen, self.zlen]
    }

    /// Cell angles in degrees.
    ///
    /// Returns `[alpha, beta, gamma]`.
    pub fn cell_angles(&self) -> [f32; 3] {
        [self.alpha, self.beta, self.gamma]
    }

    /// Logical data shape following Python `mrcfile` conventions.
    ///
    /// | Type | Shape |
    /// |------|-------|
    /// | Single image | `(1, 1, ny, nx)` |
    /// | Image stack | `(1, nz, ny, nx)` |
    /// | Volume | `(1, nz, ny, nx)` |
    /// | Volume stack | `(nz / mz, mz, ny, nx)` |
    pub fn logical_shape(&self) -> [usize; 4] {
        if self.is_volume_stack() && self.mz > 0 {
            let nvolumes = (self.nz / self.mz) as usize;
            [
                nvolumes,
                self.mz as usize,
                self.ny as usize,
                self.nx as usize,
            ]
        } else {
            [1, self.nz as usize, self.ny as usize, self.nx as usize]
        }
    }

    /// Decode header from raw bytes with correct endianness.
    ///
    /// Endianness is detected from the MACHST field and applied automatically.
    /// If the detected endianness produces an invalid MODE value, the opposite
    /// endianness is tried as a fallback (matching the behaviour of the
    /// reference Python `mrcfile` library).
    pub fn decode_from_bytes(bytes: &[u8; 1024]) -> Self {
        Self::decode_from_bytes_with_info(bytes).0
    }
}

/// Structured warning emitted when the MACHST byte-order stamp does not
/// match the actual data endianness, and the decoder had to fall back.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndianFallbackWarning {
    /// MACHST says little-endian but MODE is only valid as big-endian.
    MachstLeDataBe,
    /// MACHST says big-endian but MODE is only valid as little-endian.
    MachstBeDataLe,
}

impl core::fmt::Display for EndianFallbackWarning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MachstLeDataBe => write!(
                f,
                "MACHST indicates little-endian but MODE is valid only as \
                 big-endian; using big-endian"
            ),
            Self::MachstBeDataLe => write!(
                f,
                "MACHST indicates big-endian but MODE is valid only as \
                 little-endian; using little-endian"
            ),
        }
    }
}

impl Header {
    /// Decode header and return any byte-order fallback that occurred.
    ///
    /// Returns `(header, warning)` where `warning` is `Some` if the MACHST
    /// indicated one endianness but the MODE field was only valid under the
    /// opposite endianness.
    pub fn decode_from_bytes_with_info(
        bytes: &[u8; 1024],
    ) -> (Self, Option<EndianFallbackWarning>) {
        use crate::engine::endian::FileEndian;

        let machst = [
            bytes[OFFSET_MACHST],
            bytes[OFFSET_MACHST + 1],
            bytes[OFFSET_MACHST + 2],
            bytes[OFFSET_MACHST + 3],
        ];
        let detected = FileEndian::from_machst(&machst);

        let header = Self::decode_with_endian(bytes, detected);

        // Byte-order fallback: if MODE is invalid under detected endianness,
        // try the opposite endianness. This handles malformed files where
        // the MACHST is wrong but the rest of the file is correctly encoded.
        if crate::Mode::from_i32(header.mode).is_none() {
            let opposite = detected.opposite();
            let candidate = Self::decode_with_endian(bytes, opposite);
            if crate::Mode::from_i32(candidate.mode).is_some() {
                let warning = match detected {
                    FileEndian::LittleEndian => EndianFallbackWarning::MachstLeDataBe,
                    FileEndian::BigEndian => EndianFallbackWarning::MachstBeDataLe,
                };
                return (candidate, Some(warning));
            }
        }

        (header, None)
    }

    fn decode_with_endian(bytes: &[u8; 1024], file_endian: crate::FileEndian) -> Self {
        use crate::engine::codec::EndianCodec;

        let mut header = Self::new();

        header.nx = i32::decode(bytes, OFFSET_NX, file_endian);
        header.ny = i32::decode(bytes, OFFSET_NY, file_endian);
        header.nz = i32::decode(bytes, OFFSET_NZ, file_endian);
        header.mode = i32::decode(bytes, OFFSET_MODE, file_endian);
        header.nxstart = i32::decode(bytes, OFFSET_NXSTART, file_endian);
        header.nystart = i32::decode(bytes, OFFSET_NYSTART, file_endian);
        header.nzstart = i32::decode(bytes, OFFSET_NZSTART, file_endian);
        header.mx = i32::decode(bytes, OFFSET_MX, file_endian);
        header.my = i32::decode(bytes, OFFSET_MY, file_endian);
        header.mz = i32::decode(bytes, OFFSET_MZ, file_endian);

        header.xlen = f32::decode(bytes, OFFSET_XLEN, file_endian);
        header.ylen = f32::decode(bytes, OFFSET_YLEN, file_endian);
        header.zlen = f32::decode(bytes, OFFSET_ZLEN, file_endian);
        header.alpha = f32::decode(bytes, OFFSET_ALPHA, file_endian);
        header.beta = f32::decode(bytes, OFFSET_BETA, file_endian);
        header.gamma = f32::decode(bytes, OFFSET_GAMMA, file_endian);

        header.mapc = i32::decode(bytes, OFFSET_MAPC, file_endian);
        header.mapr = i32::decode(bytes, OFFSET_MAPR, file_endian);
        header.maps = i32::decode(bytes, OFFSET_MAPS, file_endian);

        header.dmin = f32::decode(bytes, OFFSET_DMIN, file_endian);
        header.dmax = f32::decode(bytes, OFFSET_DMAX, file_endian);
        header.dmean = f32::decode(bytes, OFFSET_DMEAN, file_endian);

        header.ispg = i32::decode(bytes, OFFSET_ISPG, file_endian);
        header.nsymbt = i32::decode(bytes, OFFSET_NSYMBT, file_endian);

        header
            .extra
            .copy_from_slice(&bytes[OFFSET_EXTRA..OFFSET_ORIGIN]);

        header.origin[0] = f32::decode(bytes, OFFSET_ORIGIN, file_endian);
        header.origin[1] = f32::decode(bytes, OFFSET_ORIGIN + 4, file_endian);
        header.origin[2] = f32::decode(bytes, OFFSET_ORIGIN + 8, file_endian);

        header
            .map
            .copy_from_slice(&bytes[OFFSET_MAP..OFFSET_MACHST]);
        header
            .machst
            .copy_from_slice(&bytes[OFFSET_MACHST..OFFSET_RMS]);

        header.rms = f32::decode(bytes, OFFSET_RMS, file_endian);
        header.nlabl = i32::decode(bytes, OFFSET_NLABL, file_endian);
        header.label.copy_from_slice(&bytes[OFFSET_LABEL..1024]);

        header
    }

    /// Encode header to raw bytes with correct endianness.
    ///
    /// Endianness is determined from the MACHST field and applied automatically.
    pub fn encode_to_bytes(&self, out: &mut [u8; 1024]) {
        use crate::engine::codec::EndianCodec;

        let file_endian = self.detect_endian();

        // Write all i32 fields
        self.nx.encode(out, OFFSET_NX, file_endian);
        self.ny.encode(out, OFFSET_NY, file_endian);
        self.nz.encode(out, OFFSET_NZ, file_endian);
        self.mode.encode(out, OFFSET_MODE, file_endian);
        self.nxstart.encode(out, OFFSET_NXSTART, file_endian);
        self.nystart.encode(out, OFFSET_NYSTART, file_endian);
        self.nzstart.encode(out, OFFSET_NZSTART, file_endian);
        self.mx.encode(out, OFFSET_MX, file_endian);
        self.my.encode(out, OFFSET_MY, file_endian);
        self.mz.encode(out, OFFSET_MZ, file_endian);

        // Write all f32 fields
        self.xlen.encode(out, OFFSET_XLEN, file_endian);
        self.ylen.encode(out, OFFSET_YLEN, file_endian);
        self.zlen.encode(out, OFFSET_ZLEN, file_endian);
        self.alpha.encode(out, OFFSET_ALPHA, file_endian);
        self.beta.encode(out, OFFSET_BETA, file_endian);
        self.gamma.encode(out, OFFSET_GAMMA, file_endian);

        // Write axis mapping fields
        self.mapc.encode(out, OFFSET_MAPC, file_endian);
        self.mapr.encode(out, OFFSET_MAPR, file_endian);
        self.maps.encode(out, OFFSET_MAPS, file_endian);

        // Write density statistics
        self.dmin.encode(out, OFFSET_DMIN, file_endian);
        self.dmax.encode(out, OFFSET_DMAX, file_endian);
        self.dmean.encode(out, OFFSET_DMEAN, file_endian);

        // Write space group and extended header size
        self.ispg.encode(out, OFFSET_ISPG, file_endian);
        self.nsymbt.encode(out, OFFSET_NSYMBT, file_endian);

        // Write extra bytes
        out[OFFSET_EXTRA..OFFSET_ORIGIN].copy_from_slice(&self.extra);

        // Write origin coordinates
        self.origin[0].encode(out, OFFSET_ORIGIN, file_endian);
        self.origin[1].encode(out, OFFSET_ORIGIN + 4, file_endian);
        self.origin[2].encode(out, OFFSET_ORIGIN + 8, file_endian);

        // Write MAP identifier - ASCII, no endian conversion
        out[OFFSET_MAP..OFFSET_MACHST].copy_from_slice(&self.map);

        // Write MACHST - byte signature, no endian conversion
        out[OFFSET_MACHST..OFFSET_RMS].copy_from_slice(&self.machst);

        // Write RMS
        self.rms.encode(out, OFFSET_RMS, file_endian);

        // Write label count
        self.nlabl.encode(out, OFFSET_NLABL, file_endian);

        // Write labels - ASCII, no endian conversion
        out[OFFSET_LABEL..1024].copy_from_slice(&self.label);
    }
}

/// Builder for constructing validated MRC headers.
///
/// # Example
/// ```
/// use mrc::HeaderBuilder;
///
/// let header = HeaderBuilder::new()
///     .shape([512, 512, 256])
///     .mode::<f32>()
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct HeaderBuilder {
    header: Header,
}

impl HeaderBuilder {
    /// Create a new header builder with sensible defaults.
    pub fn new() -> Self {
        Self {
            header: Header::new(),
        }
    }

    /// Set the volume dimensions.
    ///
    /// Also synchronises `mx`, `my`, `mz` to match `nx`, `ny`, `nz`, following
    /// the convention used by the reference Python `mrcfile` library.
    pub fn shape(mut self, shape: [usize; 3]) -> Self {
        self.header.nx = shape[0] as i32;
        self.header.ny = shape[1] as i32;
        self.header.nz = shape[2] as i32;
        self.header.mx = self.header.nx;
        self.header.my = self.header.ny;
        self.header.mz = self.header.nz;
        self
    }

    /// Set the voxel mode from a Rust type.
    pub fn mode<T: crate::mode::Voxel>(mut self) -> Self {
        self.header.mode = T::MODE.as_i32();
        self
    }

    /// Set the cell dimensions in Angstroms.
    pub fn cell_lengths(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
        self.header.xlen = xlen;
        self.header.ylen = ylen;
        self.header.zlen = zlen;
        self
    }

    /// Set the cell angles in degrees (alpha, beta, gamma).
    pub fn cell_angles(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
        self.header.alpha = alpha;
        self.header.beta = beta;
        self.header.gamma = gamma;
        self
    }

    /// Set the space group number.
    pub fn ispg(mut self, ispg: i32) -> Self {
        self.header.ispg = ispg;
        self
    }

    /// Set the extended header type (4-byte ASCII identifier).
    pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
        self.header.set_exttyp(exttyp);
        self
    }

    /// Set the extended header size in bytes.
    pub fn nsymbt(mut self, nsymbt: i32) -> Self {
        self.header.nsymbt = nsymbt;
        self
    }

    /// Set the origin coordinates.
    pub fn origin(mut self, origin: [f32; 3]) -> Self {
        self.header.origin = origin;
        self
    }

    /// Consume the builder and return the header.
    pub fn build(self) -> Result<Header, crate::HeaderValidationError> {
        self.header.validate_detailed()?;
        Ok(self.header)
    }
}

impl Default for HeaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a valid little-endian header encoded as raw bytes.
    fn le_header_bytes() -> [u8; 1024] {
        let mut h = Header::new();
        h.nx = 64;
        h.ny = 64;
        h.nz = 64;
        h.mx = 64;
        h.my = 64;
        h.mz = 64;
        h.mode = 2; // Float32
        h.set_file_endian(crate::FileEndian::LittleEndian);
        let mut bytes = [0u8; 1024];
        h.encode_to_bytes(&mut bytes);
        bytes
    }

    /// Build a valid big-endian header encoded as raw bytes.
    fn be_header_bytes() -> [u8; 1024] {
        let mut h = Header::new();
        h.nx = 64;
        h.ny = 64;
        h.nz = 64;
        h.mx = 64;
        h.my = 64;
        h.mz = 64;
        h.mode = 2; // Float32
        h.set_file_endian(crate::FileEndian::BigEndian);
        let mut bytes = [0u8; 1024];
        h.encode_to_bytes(&mut bytes);
        bytes
    }

    #[test]
    fn test_decode_roundtrip_le() {
        let original = le_header_bytes();
        let decoded = Header::decode_from_bytes(&original);
        assert_eq!(decoded.nx, 64);
        assert_eq!(decoded.mode, 2);
        assert_eq!(decoded.detect_endian(), crate::FileEndian::LittleEndian);
    }

    #[test]
    fn test_decode_roundtrip_be() {
        let original = be_header_bytes();
        let decoded = Header::decode_from_bytes(&original);
        assert_eq!(decoded.nx, 64);
        assert_eq!(decoded.mode, 2);
        assert_eq!(decoded.detect_endian(), crate::FileEndian::BigEndian);
    }

    #[test]
    fn test_byte_order_fallback_le_stamp_be_data() {
        // Create a file that claims to be LE (0x44 0x44) but is actually BE-encoded.
        let mut bytes = be_header_bytes();
        // Overwrite MACHST to claim LE
        bytes[212] = 0x44;
        bytes[213] = 0x44;
        bytes[214] = 0x00;
        bytes[215] = 0x00;

        let (decoded, warning) = Header::decode_from_bytes_with_info(&bytes);

        // Without fallback, nx would be 0x4000_0000 (garbage under LE interpretation)
        // With fallback, nx should correctly decode as 64.
        assert_eq!(
            decoded.nx, 64,
            "byte-order fallback should have corrected nx"
        );
        assert_eq!(decoded.mode, 2);
        assert!(
            warning.is_some(),
            "should emit a warning when MACHST mismatches actual byte order"
        );
    }

    #[test]
    fn test_byte_order_fallback_be_stamp_le_data() {
        // Create a file that claims to be BE (0x11 0x11) but is actually LE-encoded.
        let mut bytes = le_header_bytes();
        // Overwrite MACHST to claim BE
        bytes[212] = 0x11;
        bytes[213] = 0x11;
        bytes[214] = 0x00;
        bytes[215] = 0x00;

        let (decoded, warning) = Header::decode_from_bytes_with_info(&bytes);

        assert_eq!(
            decoded.nx, 64,
            "byte-order fallback should have corrected nx"
        );
        assert_eq!(decoded.mode, 2);
        assert!(warning.is_some());
    }

    #[test]
    fn test_no_fallback_when_machst_matches() {
        let bytes = le_header_bytes();
        let (decoded, warning) = Header::decode_from_bytes_with_info(&bytes);
        assert_eq!(decoded.nx, 64);
        assert!(warning.is_none(), "no warning when MACHST is correct");
    }

    #[test]
    fn test_ccp41_machst_recognised() {
        let mut bytes = le_header_bytes();
        bytes[212] = 0x44;
        bytes[213] = 0x41;
        let decoded = Header::decode_from_bytes(&bytes);
        assert_eq!(decoded.nx, 64);
        assert_eq!(decoded.detect_endian(), crate::FileEndian::LittleEndian);
    }

    #[test]
    fn test_nversion_le() {
        let mut h = Header::new();
        h.set_file_endian(crate::FileEndian::LittleEndian);
        assert_eq!(h.nversion(), 20141);
    }

    #[test]
    fn test_nversion_be() {
        let mut h = Header::new();
        h.set_file_endian(crate::FileEndian::BigEndian);
        assert_eq!(h.nversion(), 20141);
    }

    #[test]
    fn test_nversion_zero_accepted_by_validate() {
        // EPU files often leave NVERSION at 0 (uninitialized).
        let mut h = Header::new();
        h.nx = 64;
        h.ny = 64;
        h.nz = 1;
        h.mx = 64;
        h.my = 64;
        h.mz = 1;
        h.nlabl = 0;
        h.set_nversion(0);
        assert_eq!(h.nversion(), 0);
        assert!(h.validate(), "NVERSION=0 should pass strict validation");
    }
}
