//! Validated Header with semantic access and type conversions

use super::raw::RawHeader;
use crate::{AxisMap, Error, FileEndian, Mode};

/// Validated MRC Header with native-endian values
///
/// Created by validating a `RawHeader` and converting all fields
/// to their semantic types with proper endianness handling.
#[derive(Debug, Clone)]
pub struct Header {
    // Dimensions
    /// Number of columns (fastest varying dimension)
    pub nx: usize,
    /// Number of rows
    pub ny: usize,
    /// Number of sections (slowest varying dimension)
    pub nz: usize,
    
    // Data Type
    /// Data mode
    pub mode: Mode,
    
    // Grid Information
    /// Start position X
    pub nxstart: i32,
    /// Start position Y
    pub nystart: i32,
    /// Start position Z
    pub nzstart: i32,
    /// Grid samples X
    pub mx: i32,
    /// Grid samples Y
    pub my: i32,
    /// Grid samples Z
    pub mz: i32,
    
    // Cell Dimensions (Angstroms)
    /// Cell dimension X
    pub xlen: f32,
    /// Cell dimension Y
    pub ylen: f32,
    /// Cell dimension Z
    pub zlen: f32,
    
    // Cell Angles (degrees)
    /// Cell angle alpha
    pub alpha: f32,
    /// Cell angle beta
    pub beta: f32,
    /// Cell angle gamma
    pub gamma: f32,
    
    // Axis Mapping
    /// Axis ordering
    pub axis_map: AxisMap,
    
    // Density Statistics
    /// Minimum density
    pub dmin: f32,
    /// Maximum density
    pub dmax: f32,
    /// Mean density
    pub dmean: f32,
    
    // Space group
    /// Space group number
    pub ispg: i32,
    
    // Extended Header
    /// Extended header size in bytes
    pub nsymbt: usize,
    /// Extended header type
    pub exttyp: [u8; 4],
    /// Format version (20140 for MRC2014)
    pub nversion: i32,
    
    // File Format
    /// File endianness
    pub file_endian: FileEndian,
    /// Whether endianness was detected from MACHST (false = defaulted to little)
    pub file_endian_detected: bool,
    
    // Origin
    /// Origin X
    pub xorigin: f32,
    /// Origin Y
    pub yorigin: f32,
    /// Origin Z
    pub zorigin: f32,
    
    // RMS
    /// RMS deviation
    pub rms: f32,
    
    // Labels
    /// Number of labels used (0-10)
    pub nlabl: i32,
    /// Ten 80-character labels
    pub label: [u8; 800],
}

impl Header {
    /// Create a new header with default values
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Total number of voxels
    #[inline]
    pub fn voxel_count(&self) -> usize {
        self.nx * self.ny * self.nz
    }
    
    /// Get dimensions as tuple (nx, ny, nz)
    #[inline]
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.nx, self.ny, self.nz)
    }
    
    /// Get voxel size in Angstroms (dx, dy, dz)
    #[inline]
    pub fn voxel_size(&self) -> (f32, f32, f32) {
        let dx = if self.nx > 0 { self.xlen / self.nx as f32 } else { 0.0 };
        let dy = if self.ny > 0 { self.ylen / self.ny as f32 } else { 0.0 };
        let dz = if self.nz > 0 { self.zlen / self.nz as f32 } else { 0.0 };
        (dx, dy, dz)
    }
    
    /// Calculate data size in bytes
    pub fn data_size(&self) -> usize {
        let voxel_count = self.voxel_count();
        match self.mode {
            Mode::Int8 => voxel_count,
            Mode::Int16 | Mode::Uint16 | Mode::Float16 => voxel_count * 2,
            Mode::Float32 | Mode::Int16Complex => voxel_count * 4,
            Mode::Float32Complex => voxel_count * 8,
            Mode::Packed4Bit => voxel_count.div_ceil(2),
        }
    }
    
    /// Calculate total file size
    pub fn file_size(&self) -> usize {
        RawHeader::SIZE + self.nsymbt + self.data_size()
    }
    
    /// Calculate data offset in file
    pub fn data_offset(&self) -> usize {
        RawHeader::SIZE + self.nsymbt
    }
    
    /// Get EXTTYP as a string
    pub fn exttyp_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.exttyp)
    }
    
    /// Get a label as a string (returns None if index out of range)
    pub fn label_str(&self, index: usize) -> Option<&str> {
        if index >= 10 || index >= self.nlabl as usize {
            return None;
        }
        let start = index * 80;
        let label_slice = &self.label[start..start + 80];
        // Trim trailing spaces and nulls
        let end = label_slice.iter()
            .rposition(|&b| b != b' ' && b != 0)
            .map(|i| i + 1)
            .unwrap_or(0);
        core::str::from_utf8(&label_slice[..end]).ok()
    }
    
    // --- Setters with chaining support ---
    
    /// Set EXTTYP from bytes
    pub fn set_exttyp(&mut self, value: [u8; 4]) -> &mut Self {
        self.exttyp = value;
        self
    }
    
    /// Set EXTTYP from string
    pub fn set_exttyp_str(&mut self, value: &str) -> Result<&mut Self, &'static str> {
        if value.len() != 4 {
            return Err("EXTTYP must be exactly 4 characters");
        }
        let bytes = value.as_bytes();
        self.exttyp.copy_from_slice(bytes);
        Ok(self)
    }
    
    /// Set NVERSION
    pub fn set_nversion(&mut self, value: i32) -> &mut Self {
        self.nversion = value;
        self
    }
    
    /// Set axis mapping
    pub fn set_axis_map(&mut self, mapc: i32, mapr: i32, maps: i32) -> Result<&mut Self, Error> {
        self.axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        Ok(self)
    }
    
    /// Set dimensions
    pub fn set_dimensions(&mut self, nx: usize, ny: usize, nz: usize) -> &mut Self {
        self.nx = nx;
        self.ny = ny;
        self.nz = nz;
        self
    }
    
    /// Set cell dimensions
    pub fn set_cell_dimensions(&mut self, xlen: f32, ylen: f32, zlen: f32) -> &mut Self {
        self.xlen = xlen;
        self.ylen = ylen;
        self.zlen = zlen;
        self
    }
    
    /// Set origin coordinates
    pub fn set_origin(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.xorigin = x;
        self.yorigin = y;
        self.zorigin = z;
        self
    }
    
    /// Set density statistics
    pub fn set_statistics(&mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) -> &mut Self {
        self.dmin = dmin;
        self.dmax = dmax;
        self.dmean = dmean;
        self.rms = rms;
        self
    }
    
    /// Set mode
    pub fn set_mode(&mut self, mode: Mode) -> &mut Self {
        self.mode = mode;
        self
    }
    
    /// Create a header builder
    pub fn builder() -> HeaderBuilder {
        HeaderBuilder::new()
    }
}

/// Decode a value that was already loaded into memory via bytemuck
/// 
/// When using bytemuck to cast raw file bytes to a struct, the struct fields
/// contain the raw bit patterns from the file. This function reinterprets
/// those bits with the correct file endianness.
#[inline]
fn convert_i32_from_file_endian(value: i32, file_endian: FileEndian) -> i32 {
    let bytes = value.to_ne_bytes(); // Get raw bytes as stored in memory
    match file_endian {
        FileEndian::Little => i32::from_le_bytes(bytes),
        FileEndian::Big => i32::from_be_bytes(bytes),
    }
}

#[inline]
fn convert_f32_from_file_endian(value: f32, file_endian: FileEndian) -> f32 {
    let bits = value.to_bits();
    let bytes = bits.to_ne_bytes(); // Get raw bytes as stored in memory
    let converted = match file_endian {
        FileEndian::Little => u32::from_le_bytes(bytes),
        FileEndian::Big => u32::from_be_bytes(bytes),
    };
    f32::from_bits(converted)
}

impl TryFrom<RawHeader> for Header {
    type Error = Error;
    
    fn try_from(raw: RawHeader) -> Result<Self, Self::Error> {
        // Detect file endianness from MACHST, default to little-endian if unknown
        let (file_endian, detected) = FileEndian::from_machst_or_little(&raw.machst);
        
        // Convert all multi-byte fields from file endianness to native
        // Note: RawHeader fields contain raw bit patterns from file
        let nx = convert_i32_from_file_endian(raw.nx, file_endian);
        let ny = convert_i32_from_file_endian(raw.ny, file_endian);
        let nz = convert_i32_from_file_endian(raw.nz, file_endian);
        
        // Validate dimensions with overflow check
        if nx <= 0 || ny <= 0 || nz <= 0 {
            return Err(Error::InvalidDimensions);
        }
        
        // OOM protection: check for reasonable dimensions
        let voxel_count = (nx as usize)
            .checked_mul(ny as usize)
            .and_then(|v| v.checked_mul(nz as usize))
            .ok_or(Error::InvalidDimensions)?;
        
        // Sanity check: prevent absurdly large files (> 1TB)
        const MAX_REASONABLE_VOXELS: usize = 256 * 1024 * 1024 * 1024; // ~1B voxels for f32 = 4GB
        if voxel_count > MAX_REASONABLE_VOXELS {
            return Err(Error::InvalidDimensions);
        }
        
        // Decode and validate mode
        let mode_val = convert_i32_from_file_endian(raw.mode, file_endian);
        let mode = Mode::try_from(mode_val).map_err(|_| Error::InvalidMode)?;
        
        // Validate mode 12 requires f16 feature
        #[cfg(not(feature = "f16"))]
        if mode == Mode::Float16 {
            return Err(Error::FeatureDisabled { feature: "f16" });
        }
        
        // Decode axis mapping
        let mapc = convert_i32_from_file_endian(raw.mapc, file_endian);
        let mapr = convert_i32_from_file_endian(raw.mapr, file_endian);
        let maps = convert_i32_from_file_endian(raw.maps, file_endian);
        let axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        
        // Decode nsymbt with OOM protection
        let nsymbt = convert_i32_from_file_endian(raw.nsymbt, file_endian) as usize;
        const MAX_EXTENDED_HEADER: usize = 1024 * 1024 * 1024; // 1GB max extended header
        if nsymbt > MAX_EXTENDED_HEADER {
            return Err(Error::InvalidDimensions);
        }
        
        // Decode nversion from extra bytes
        let nversion = raw.nversion(file_endian);
        
        // Get nlabl with validation
        let nlabl = convert_i32_from_file_endian(raw.nlabl, file_endian);
        if !(0..=10).contains(&nlabl) {
            return Err(Error::InvalidHeader);
        }
        
        Ok(Self {
            nx: nx as usize,
            ny: ny as usize,
            nz: nz as usize,
            mode,
            nxstart: convert_i32_from_file_endian(raw.nxstart, file_endian),
            nystart: convert_i32_from_file_endian(raw.nystart, file_endian),
            nzstart: convert_i32_from_file_endian(raw.nzstart, file_endian),
            mx: convert_i32_from_file_endian(raw.mx, file_endian),
            my: convert_i32_from_file_endian(raw.my, file_endian),
            mz: convert_i32_from_file_endian(raw.mz, file_endian),
            xlen: convert_f32_from_file_endian(raw.xlen, file_endian),
            ylen: convert_f32_from_file_endian(raw.ylen, file_endian),
            zlen: convert_f32_from_file_endian(raw.zlen, file_endian),
            alpha: convert_f32_from_file_endian(raw.alpha, file_endian),
            beta: convert_f32_from_file_endian(raw.beta, file_endian),
            gamma: convert_f32_from_file_endian(raw.gamma, file_endian),
            axis_map,
            dmin: convert_f32_from_file_endian(raw.dmin, file_endian),
            dmax: convert_f32_from_file_endian(raw.dmax, file_endian),
            dmean: convert_f32_from_file_endian(raw.dmean, file_endian),
            ispg: convert_i32_from_file_endian(raw.ispg, file_endian),
            nsymbt,
            exttyp: raw.exttyp(),
            nversion,
            file_endian,
            file_endian_detected: detected,
            xorigin: convert_f32_from_file_endian(raw.origin[0], file_endian),
            yorigin: convert_f32_from_file_endian(raw.origin[1], file_endian),
            zorigin: convert_f32_from_file_endian(raw.origin[2], file_endian),
            rms: convert_f32_from_file_endian(raw.rms, file_endian),
            nlabl,
            label: raw.label,
        })
    }
}

/// Encode a value to file endianness
#[inline]
fn encode_i32_to_file_endian(value: i32, endian: FileEndian) -> i32 {
    match endian {
        FileEndian::Little => value.to_le(),
        FileEndian::Big => value.to_be(),
    }
}

#[inline]
fn encode_f32_to_file_endian(value: f32, endian: FileEndian) -> f32 {
    let bits = value.to_bits();
    let converted = match endian {
        FileEndian::Little => bits.to_le(),
        FileEndian::Big => bits.to_be(),
    };
    f32::from_bits(converted)
}

impl From<Header> for RawHeader {
    fn from(header: Header) -> Self {
        let mut raw = RawHeader::new();
        
        raw.nx = encode_i32_to_file_endian(header.nx as i32, header.file_endian);
        raw.ny = encode_i32_to_file_endian(header.ny as i32, header.file_endian);
        raw.nz = encode_i32_to_file_endian(header.nz as i32, header.file_endian);
        raw.mode = encode_i32_to_file_endian(header.mode.into(), header.file_endian);
        raw.nxstart = encode_i32_to_file_endian(header.nxstart, header.file_endian);
        raw.nystart = encode_i32_to_file_endian(header.nystart, header.file_endian);
        raw.nzstart = encode_i32_to_file_endian(header.nzstart, header.file_endian);
        raw.mx = encode_i32_to_file_endian(header.mx, header.file_endian);
        raw.my = encode_i32_to_file_endian(header.my, header.file_endian);
        raw.mz = encode_i32_to_file_endian(header.mz, header.file_endian);
        raw.xlen = encode_f32_to_file_endian(header.xlen, header.file_endian);
        raw.ylen = encode_f32_to_file_endian(header.ylen, header.file_endian);
        raw.zlen = encode_f32_to_file_endian(header.zlen, header.file_endian);
        raw.alpha = encode_f32_to_file_endian(header.alpha, header.file_endian);
        raw.beta = encode_f32_to_file_endian(header.beta, header.file_endian);
        raw.gamma = encode_f32_to_file_endian(header.gamma, header.file_endian);
        raw.mapc = encode_i32_to_file_endian(header.axis_map.column as i32, header.file_endian);
        raw.mapr = encode_i32_to_file_endian(header.axis_map.row as i32, header.file_endian);
        raw.maps = encode_i32_to_file_endian(header.axis_map.section as i32, header.file_endian);
        raw.dmin = encode_f32_to_file_endian(header.dmin, header.file_endian);
        raw.dmax = encode_f32_to_file_endian(header.dmax, header.file_endian);
        raw.dmean = encode_f32_to_file_endian(header.dmean, header.file_endian);
        raw.ispg = encode_i32_to_file_endian(header.ispg, header.file_endian);
        raw.nsymbt = encode_i32_to_file_endian(header.nsymbt as i32, header.file_endian);
        raw.set_exttyp(header.exttyp);
        raw.set_nversion(header.nversion, header.file_endian);
        raw.machst = header.file_endian.to_machst();
        raw.origin = [
            encode_f32_to_file_endian(header.xorigin, header.file_endian),
            encode_f32_to_file_endian(header.yorigin, header.file_endian),
            encode_f32_to_file_endian(header.zorigin, header.file_endian),
        ];
        raw.rms = encode_f32_to_file_endian(header.rms, header.file_endian);
        raw.nlabl = encode_i32_to_file_endian(header.nlabl, header.file_endian);
        raw.label = header.label;
        
        raw
    }
}

impl Default for Header {
    fn default() -> Self {
        Self {
            nx: 1,
            ny: 1,
            nz: 1,
            mode: Mode::Float32,
            nxstart: 0,
            nystart: 0,
            nzstart: 0,
            mx: 1,
            my: 1,
            mz: 1,
            xlen: 1.0,
            ylen: 1.0,
            zlen: 1.0,
            alpha: 90.0,
            beta: 90.0,
            gamma: 90.0,
            axis_map: AxisMap::default(),
            dmin: 0.0,
            dmax: 0.0,
            dmean: 0.0,
            ispg: 0,
            nsymbt: 0,
            exttyp: [0; 4],
            nversion: 20140,
            file_endian: FileEndian::native(),
            file_endian_detected: true,
            xorigin: 0.0,
            yorigin: 0.0,
            zorigin: 0.0,
            rms: 0.0,
            nlabl: 0,
            label: [0; 800],
        }
    }
}

/// Builder for constructing MRC headers
#[derive(Debug)]
pub struct HeaderBuilder {
    header: Header,
}

impl HeaderBuilder {
    /// Create a new header builder with default values
    pub fn new() -> Self {
        Self {
            header: Header::new(),
        }
    }
    
    /// Set the dimensions (nx, ny, nz)
    pub fn dimensions(mut self, nx: usize, ny: usize, nz: usize) -> Self {
        self.header.set_dimensions(nx, ny, nz);
        self
    }
    
    /// Set the mode
    pub fn mode(mut self, mode: Mode) -> Self {
        self.header.set_mode(mode);
        self
    }
    
    /// Set the cell dimensions in Angstroms (xlen, ylen, zlen)
    pub fn cell_dimensions(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
        self.header.set_cell_dimensions(xlen, ylen, zlen);
        self
    }
    
    /// Set the cell angles in degrees (alpha, beta, gamma)
    pub fn cell_angles(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
        self.header.alpha = alpha;
        self.header.beta = beta;
        self.header.gamma = gamma;
        self
    }
    
    /// Set the axis mapping (mapc, mapr, maps)
    pub fn axis_mapping(mut self, mapc: i32, mapr: i32, maps: i32) -> Self {
        let _ = self.header.set_axis_map(mapc, mapr, maps);
        self
    }
    
    /// Set the origin coordinates
    pub fn origin(mut self, x: f32, y: f32, z: f32) -> Self {
        self.header.set_origin(x, y, z);
        self
    }
    
    /// Set the density statistics
    pub fn statistics(mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) -> Self {
        self.header.set_statistics(dmin, dmax, dmean, rms);
        self
    }
    
    /// Set the space group number
    pub fn space_group(mut self, ispg: i32) -> Self {
        self.header.ispg = ispg;
        self
    }
    
    /// Set the extended header size in bytes
    pub fn extended_header_size(mut self, nsymbt: usize) -> Self {
        self.header.nsymbt = nsymbt;
        self
    }
    
    /// Set the EXTTYP identifier
    pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
        self.header.set_exttyp(exttyp);
        self
    }
    
    /// Set the NVERSION number
    pub fn nversion(mut self, nversion: i32) -> Self {
        self.header.set_nversion(nversion);
        self
    }
    
    /// Build the header
    pub fn build(self) -> Header {
        self.header
    }
}

impl Default for HeaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}