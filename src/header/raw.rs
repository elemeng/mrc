//! Raw 1024-byte MRC header with exact binary layout
//!
//! This is a direct memory mapping of the MRC2014 header format.
//! All fields are stored as they appear in the file (file-endian).

use crate::mode::Mode;

/// Raw 1024-byte MRC header with exact binary layout
///
/// Field offsets follow the MRC2014 specification.
/// This struct is Pod and Zeroable - it can be safely cast from/to bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
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
    
    // --- Bytes 96-99: Extended header type ---
    /// Extended header type identifier (e.g., "CCP4")
    pub exttyp: [u8; 4],
    
    // --- Bytes 100-103: Version ---
    /// Format version (20140 for MRC2014)
    pub nversion: i32,
    
    // --- Bytes 104-107: Reserved ---
    _pad1: i32,
    
    // --- Bytes 108-119: Origin ---
    /// Origin coordinates
    pub origin: [f32; 3],
    
    // --- Bytes 120-127: Reserved ---
    _pad2: [f32; 2],
    
    // --- Bytes 128-131: RMS ---
    /// RMS deviation from mean
    pub rms: f32,
    
    // --- Bytes 132-211: Reserved ---
    _pad3: [u8; 80],
    
    // --- Bytes 212-215: Machine stamp ---
    /// Machine stamp indicating endianness
    pub machst: [u8; 4],
    
    // --- Bytes 216-219: Reserved ---
    _pad4: [u8; 4],
    
    // --- Bytes 220-223: Label count ---
    /// Number of labels used (0-10)
    pub nlabl: i32,
    
    // --- Bytes 224-227: MAP identifier ---
    /// Must be "MAP " for valid MRC file
    pub map: [u8; 4],
    
    // --- Bytes 228-1023: Labels ---
    /// Ten 80-character labels
    pub label: [[u8; 80]; 10],
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
        let mut header = Self::zeroed();
        header.nversion = 20140;
        header.map = *b"MAP ";
        header.machst = [0x44, 0x44, 0x00, 0x00]; // Little-endian
        header.mapc = 1;
        header.mapr = 2;
        header.maps = 3;
        header
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
        let n = (self.nx as usize)
            .saturating_mul(self.ny as usize)
            .saturating_mul(self.nz as usize);
        
        if n == 0 {
            return 0;
        }
        
        let bytes_per_voxel = match self.mode {
            0 | 101 => 1,
            1 | 3 | 6 | 12 => 2,
            2 => 4,
            4 => 8,
            _ => 4,
        };
        
        if self.mode == 101 {
            n.div_ceil(2)
        } else {
            n.saturating_mul(bytes_per_voxel)
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
        &self.map == b"MAP " || &self.map[..3] == b"MAP" || self.map == [0; 4]
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
}

impl Default for RawHeader {
    fn default() -> Self {
        Self::new()
    }
}