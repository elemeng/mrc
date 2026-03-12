//! Raw 1024-byte MRC header with exact binary layout
//!
//! This is a direct memory mapping of the MRC2014 header format.
//! All fields are stored as they appear in the file (file-endian).

use crate::mode::Mode;

/// Raw 1024-byte MRC header with exact binary layout
///
/// Field offsets follow the MRC2014 specification.
/// This struct is Pod and Zeroable - it can be safely cast from/to bytes.
#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawHeader {
    // --- Bytes 0-11: Dimensions ---
    /// Number of columns (NX) - fastest varying
    pub nx: i32,
    /// Number of rows (NY)
    pub ny: i32,
    /// Number of sections (NZ) - slowest varying
    pub nz: i32,
    
    // --- Bytes 12-15: Mode ---
    /// Data type mode (0=i8, 1=i16, 2=f32, 3=ci16, 4=cf32, 6=u16, 12=f16, 101=packed)
    pub mode: i32,
    
    // --- Bytes 16-27: Start positions ---
    /// Start of columns in unit cell
    pub nxstart: i32,
    /// Start of rows in unit cell
    pub nystart: i32,
    /// Start of sections in unit cell
    pub nzstart: i32,
    
    // --- Bytes 28-39: Grid sampling ---
    /// Number of samples along X
    pub mx: i32,
    /// Number of samples along Y
    pub my: i32,
    /// Number of samples along Z
    pub mz: i32,
    
    // --- Bytes 40-51: Cell dimensions ---
    /// Cell dimension X (Angstroms)
    pub xlen: f32,
    /// Cell dimension Y (Angstroms)
    pub ylen: f32,
    /// Cell dimension Z (Angstroms)
    pub zlen: f32,
    
    // --- Bytes 52-63: Cell angles ---
    /// Cell angle alpha (degrees)
    pub alpha: f32,
    /// Cell angle beta (degrees)
    pub beta: f32,
    /// Cell angle gamma (degrees)
    pub gamma: f32,
    
    // --- Bytes 64-75: Axis mapping ---
    /// Column axis (1=X, 2=Y, 3=Z)
    pub mapc: i32,
    /// Row axis (1=X, 2=Y, 3=Z)
    pub mapr: i32,
    /// Section axis (1=X, 2=Y, 3=Z)
    pub maps: i32,
    
    // --- Bytes 76-87: Statistics ---
    /// Minimum density value
    pub dmin: f32,
    /// Maximum density value
    pub dmax: f32,
    /// Mean density value
    pub dmean: f32,
    
    // --- Bytes 88-91: Space group ---
    /// Space group number (0 for image stacks)
    pub ispg: i32,
    
    // --- Bytes 92-95: Extended header size ---
    /// Size of extended header in bytes
    pub nsymbt: i32,
    
    // --- Bytes 96-195: Extra space (100 bytes) ---
    /// Extra space: bytes 8-11 hold EXTTYP, bytes 12-15 hold NVERSION
    pub extra: [u8; 100],
    
    // --- Bytes 196-207: Origin ---
    /// Origin coordinates (X, Y, Z)
    pub origin: [f32; 3],
    
    // --- Bytes 208-211: MAP identifier ---
    /// Must be "MAP " for valid MRC file
    pub map: [u8; 4],
    
    // --- Bytes 212-215: Machine stamp ---
    /// Machine stamp indicating endianness
    pub machst: [u8; 4],
    
    // --- Bytes 216-219: RMS ---
    /// RMS deviation from mean
    pub rms: f32,
    
    // --- Bytes 220-223: Label count ---
    /// Number of labels used (0-10)
    pub nlabl: i32,
    
    // --- Bytes 224-1023: Labels ---
    /// Ten 80-character labels (800 bytes total)
    pub label: [u8; 800],
}

// Safety: RawHeader is repr(C), has no padding bytes that could be uninitialized,
// and all fields are Pod types (i32, f32, [u8; N]).
unsafe impl bytemuck::Pod for RawHeader {}
unsafe impl bytemuck::Zeroable for RawHeader {}

impl RawHeader {
    /// Header size in bytes
    pub const SIZE: usize = 1024;
    
    /// Create a new header with default values
    pub fn new() -> Self {
        Self {
            nx: 0,
            ny: 0,
            nz: 0,
            mode: 2, // 32-bit float
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
            machst: [0x44, 0x44, 0x00, 0x00], // Little-endian
            rms: -1.0,
            nlabl: 0,
            label: [0; 800],
        }
    }
    
    /// Create a zeroed header
    pub fn zeroed() -> Self {
        // SAFETY: RawHeader is Zeroable
        unsafe { core::mem::zeroed() }
    }
    
    /// Get the data mode
    pub fn mode(&self) -> Option<Mode> {
        Mode::from_i32(self.mode)
    }
    
    /// Calculate total data size in bytes
    pub fn data_size(&self) -> usize {
        let nx = self.nx.max(0) as usize;
        let ny = self.ny.max(0) as usize;
        let nz = self.nz.max(0) as usize;
        let voxel_count = nx * ny * nz;
        
        match Mode::from_i32(self.mode) {
            Some(Mode::Packed4Bit) => voxel_count.div_ceil(2),
            Some(mode) => voxel_count * mode.byte_size(),
            None => voxel_count * 4, // Default to 4 bytes for unknown modes
        }
    }
    
    /// Calculate total file size
    pub fn file_size(&self) -> usize {
        Self::SIZE.saturating_add(self.nsymbt as usize).saturating_add(self.data_size())
    }
    
    /// Calculate data offset in file
    pub fn data_offset(&self) -> usize {
        Self::SIZE + (self.nsymbt as usize)
    }
    
    /// Check if MAP field is valid
    pub fn has_valid_map(&self) -> bool {
        // Standard MRC2014 format
        if &self.map == b"MAP " {
            return true;
        }
        // Accept legacy variants: "MAP\0" or "MAPI"
        if &self.map[..3] == b"MAP" && (self.map[3] == b' ' || self.map[3] == 0 || self.map[3] == b'I') {
            return true;
        }
        // Accept all zeros (uninitialized, common in some generated files)
        self.map == [0; 4]
    }
    
    /// Check if mode is valid
    pub fn is_valid_mode(&self) -> bool {
        matches!(self.mode, 0 | 1 | 2 | 3 | 4 | 6 | 12 | 101)
    }
    
    /// Validate dimensions
    pub fn has_valid_dimensions(&self) -> bool {
        self.nx > 0 && self.ny > 0 && self.nz > 0
    }
    
    /// Validate axis mapping is a permutation of 1, 2, 3
    pub fn has_valid_axis_map(&self) -> bool {
        let axes = [self.mapc, self.mapr, self.maps];
        axes.iter().all(|&a| (1..=3).contains(&a))
            && axes[0] != axes[1]
            && axes[1] != axes[2]
            && axes[0] != axes[2]
    }
    
    /// Full validation of the header
    pub fn validate(&self) -> bool {
        self.has_valid_dimensions()
            && self.is_valid_mode()
            && self.has_valid_map()
            && self.has_valid_axis_map()
            && self.nsymbt >= 0
            && self.nlabl >= 0 && self.nlabl <= 10
    }
    
    // --- EXTTYP accessors ---
    
    /// Get the EXTTYP identifier (bytes 96-99, stored in extra[8..12])
    pub fn exttyp(&self) -> [u8; 4] {
        [self.extra[8], self.extra[9], self.extra[10], self.extra[11]]
    }
    
    /// Set the EXTTYP identifier
    pub fn set_exttyp(&mut self, value: [u8; 4]) {
        self.extra[8..12].copy_from_slice(&value);
    }
    
    /// Get EXTTYP as a string
    pub fn exttyp_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.extra[8..12])
    }
    
    // --- NVERSION accessors ---
    
    /// Get the NVERSION (bytes 100-103, stored in extra[12..16])
    /// Note: This respects file endianness
    pub fn nversion(&self, endian: crate::FileEndian) -> i32 {
        let bytes = [self.extra[12], self.extra[13], self.extra[14], self.extra[15]];
        match endian {
            crate::FileEndian::Little => i32::from_le_bytes(bytes),
            crate::FileEndian::Big => i32::from_be_bytes(bytes),
        }
    }
    
    /// Set the NVERSION
    pub fn set_nversion(&mut self, value: i32, endian: crate::FileEndian) {
        let bytes = match endian {
            crate::FileEndian::Little => value.to_le_bytes(),
            crate::FileEndian::Big => value.to_be_bytes(),
        };
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
    
    #[test]
    fn test_field_offsets() {
        // Verify critical field offsets match MRC2014 spec
        let header = RawHeader::new();
        let base = &header as *const _ as usize;
        
        // Helper to get offset
        let offset = |field: *const u8| field as usize - base;
        
        // Check dimensions (0-11)
        assert_eq!(offset(&header.nx as *const _ as *const u8), 0);
        assert_eq!(offset(&header.ny as *const _ as *const u8), 4);
        assert_eq!(offset(&header.nz as *const _ as *const u8), 8);
        
        // Mode (12-15)
        assert_eq!(offset(&header.mode as *const _ as *const u8), 12);
        
        // Start positions (16-27)
        assert_eq!(offset(&header.nxstart as *const _ as *const u8), 16);
        
        // Grid sampling (28-39)
        assert_eq!(offset(&header.mx as *const _ as *const u8), 28);
        
        // Cell dimensions (40-51)
        assert_eq!(offset(&header.xlen as *const _ as *const u8), 40);
        
        // Cell angles (52-63)
        assert_eq!(offset(&header.alpha as *const _ as *const u8), 52);
        
        // Axis mapping (64-75)
        assert_eq!(offset(&header.mapc as *const _ as *const u8), 64);
        
        // Statistics (76-87)
        assert_eq!(offset(&header.dmin as *const _ as *const u8), 76);
        
        // Space group (88-91)
        assert_eq!(offset(&header.ispg as *const _ as *const u8), 88);
        
        // Extended header size (92-95)
        assert_eq!(offset(&header.nsymbt as *const _ as *const u8), 92);
        
        // Extra space (96-195)
        assert_eq!(offset(&header.extra[0] as *const u8), 96);
        
        // Origin (196-207)
        assert_eq!(offset(&header.origin[0] as *const f32 as *const u8), 196);
        
        // MAP (208-211)
        assert_eq!(offset(&header.map[0] as *const u8), 208);
        
        // MACHST (212-215)
        assert_eq!(offset(&header.machst[0] as *const u8), 212);
        
        // RMS (216-219)
        assert_eq!(offset(&header.rms as *const _ as *const u8), 216);
        
        // NLABL (220-223)
        assert_eq!(offset(&header.nlabl as *const _ as *const u8), 220);
        
        // LABEL (224-1023)
        assert_eq!(offset(&header.label[0] as *const u8), 224);
    }
    
    #[test]
    fn test_new_header_defaults() {
        let h = RawHeader::new();
        assert_eq!(h.map, *b"MAP ");
        assert_eq!(h.machst, [0x44, 0x44, 0x00, 0x00]);
        assert_eq!(h.mapc, 1);
        assert_eq!(h.mapr, 2);
        assert_eq!(h.maps, 3);
        assert!(h.has_valid_map());
        assert!(h.has_valid_axis_map());
    }
}
