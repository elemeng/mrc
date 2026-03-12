//! Validated Header with semantic access and type conversions

use super::raw::RawHeader;
use crate::{AxisMap, Error, FileEndian, Mode};

/// Validated MRC Header with native-endian values
///
/// Created by validating a `RawHeader` and converting all fields
/// to their semantic types with proper endianness handling.
///
/// This struct composes `RawHeader` to avoid field duplication.
/// Access raw fields via deref, or use convenience accessors for
/// converted types (nx/ny/nz as usize, mode as Mode, etc.)
#[derive(Debug, Clone)]
pub struct Header {
    /// The raw header - all fields stored in file format
    pub raw: RawHeader,
    
    /// Computed axis mapping (from mapc/mapr/maps)
    pub axis_map: AxisMap,
    
    /// File endianness
    pub file_endian: FileEndian,
    
    /// Whether endianness was detected from MACHST
    pub file_endian_detected: bool,
}

impl Header {
    /// Create a new header with default values
    pub fn new() -> Self {
        Self::default()
    }
    
    // === Dimension accessors (usize for convenience) ===
    
    /// Number of columns (fastest varying dimension)
    #[inline]
    pub fn nx(&self) -> usize {
        self.raw.nx.max(0) as usize
    }
    
    /// Number of rows
    #[inline]
    pub fn ny(&self) -> usize {
        self.raw.ny.max(0) as usize
    }
    
    /// Number of sections (slowest varying dimension)
    #[inline]
    pub fn nz(&self) -> usize {
        self.raw.nz.max(0) as usize
    }
    
    // === Setters for dimensions ===
    
    /// Set dimensions
    pub fn set_dimensions(&mut self, nx: usize, ny: usize, nz: usize) -> &mut Self {
        self.raw.nx = nx as i32;
        self.raw.ny = ny as i32;
        self.raw.nz = nz as i32;
        self
    }
    
    // === Mode accessor ===
    
    /// Data mode
    #[inline]
    pub fn mode(&self) -> Mode {
        // RawHeader stores mode in native endian after conversion
        Mode::from_i32(self.raw.mode).unwrap_or(Mode::Float32)
    }
    
    /// Set mode
    pub fn set_mode(&mut self, mode: Mode) -> &mut Self {
        self.raw.mode = mode as i32;
        self
    }
    
    // === Extended header ===
    
    /// Extended header size in bytes
    #[inline]
    pub fn nsymbt(&self) -> usize {
        self.raw.nsymbt.max(0) as usize
    }
    
    /// Set extended header size
    pub fn set_nsymbt(&mut self, nsymbt: usize) -> &mut Self {
        self.raw.nsymbt = nsymbt as i32;
        self
    }
    
    // === Origin accessors ===
    
    /// Origin X
    #[inline]
    pub fn xorigin(&self) -> f32 {
        self.raw.origin[0]
    }
    
    /// Origin Y
    #[inline]
    pub fn yorigin(&self) -> f32 {
        self.raw.origin[1]
    }
    
    /// Origin Z
    #[inline]
    pub fn zorigin(&self) -> f32 {
        self.raw.origin[2]
    }
    
    /// Set origin coordinates
    pub fn set_origin(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.raw.origin = [x, y, z];
        self
    }
    
    // === EXTTYP accessors ===
    
    /// Extended header type
    #[inline]
    pub fn exttyp(&self) -> [u8; 4] {
        self.raw.exttyp()
    }
    
    /// Set EXTTYP from bytes
    pub fn set_exttyp(&mut self, value: [u8; 4]) -> &mut Self {
        self.raw.set_exttyp(value);
        self
    }
    
    /// Set EXTTYP from string
    pub fn set_exttyp_str(&mut self, value: &str) -> Result<&mut Self, &'static str> {
        if value.len() != 4 {
            return Err("EXTTYP must be exactly 4 characters");
        }
        self.raw.extra[8..12].copy_from_slice(value.as_bytes());
        Ok(self)
    }
    
    /// Get EXTTYP as a string
    pub fn exttyp_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.raw.extra[8..12])
    }
    
    // === NVERSION accessors ===
    
    /// Format version (20140 for MRC2014)
    #[inline]
    pub fn nversion(&self) -> i32 {
        self.raw.nversion(self.file_endian)
    }
    
    /// Set NVERSION
    pub fn set_nversion(&mut self, value: i32) -> &mut Self {
        self.raw.set_nversion(value, self.file_endian);
        self
    }
    
    // === Label accessors ===
    
    /// Number of labels used (0-10)
    #[inline]
    pub fn nlabl(&self) -> i32 {
        self.raw.nlabl
    }
    
    /// Set number of labels
    pub fn set_nlabl(&mut self, nlabl: i32) -> &mut Self {
        self.raw.nlabl = nlabl;
        self
    }
    
    /// Get a label as a string (returns None if index out of range)
    pub fn label_str(&self, index: usize) -> Option<&str> {
        if index >= 10 || index >= self.nlabl() as usize {
            return None;
        }
        let start = index * 80;
        let label_slice = &self.raw.label[start..start + 80];
        // Trim trailing spaces and nulls
        let end = label_slice.iter()
            .rposition(|&b| b != b' ' && b != 0)
            .map(|i| i + 1)
            .unwrap_or(0);
        core::str::from_utf8(&label_slice[..end]).ok()
    }
    
    // === Cell dimensions ===
    
    /// Set cell dimensions
    pub fn set_cell_dimensions(&mut self, xlen: f32, ylen: f32, zlen: f32) -> &mut Self {
        self.raw.xlen = xlen;
        self.raw.ylen = ylen;
        self.raw.zlen = zlen;
        self
    }
    
    // === Statistics ===
    
    /// Set density statistics
    pub fn set_statistics(&mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) -> &mut Self {
        self.raw.dmin = dmin;
        self.raw.dmax = dmax;
        self.raw.dmean = dmean;
        self.raw.rms = rms;
        self
    }
    
    // === Axis mapping ===
    
    /// Set axis mapping
    pub fn set_axis_map(&mut self, mapc: i32, mapr: i32, maps: i32) -> Result<&mut Self, Error> {
        self.axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        self.raw.mapc = mapc;
        self.raw.mapr = mapr;
        self.raw.maps = maps;
        Ok(self)
    }
    
    // === Computed properties ===
    
    /// Total number of voxels
    #[inline]
    pub fn voxel_count(&self) -> usize {
        self.nx() * self.ny() * self.nz()
    }
    
    /// Get dimensions as tuple (nx, ny, nz)
    #[inline]
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.nx(), self.ny(), self.nz())
    }
    
    /// Get voxel size in Angstroms (dx, dy, dz)
    #[inline]
    pub fn voxel_size(&self) -> (f32, f32, f32) {
        let nx = self.nx();
        let ny = self.ny();
        let nz = self.nz();
        let dx = if nx > 0 { self.raw.xlen / nx as f32 } else { 0.0 };
        let dy = if ny > 0 { self.raw.ylen / ny as f32 } else { 0.0 };
        let dz = if nz > 0 { self.raw.zlen / nz as f32 } else { 0.0 };
        (dx, dy, dz)
    }
    
    /// Calculate data size in bytes
    pub fn data_size(&self) -> usize {
        let voxel_count = self.voxel_count();
        match self.mode() {
            Mode::Int8 => voxel_count,
            Mode::Int16 | Mode::Uint16 | Mode::Float16 => voxel_count * 2,
            Mode::Float32 | Mode::Int16Complex => voxel_count * 4,
            Mode::Float32Complex => voxel_count * 8,
            Mode::Packed4Bit => voxel_count.div_ceil(2),
        }
    }
    
    /// Calculate total file size
    pub fn file_size(&self) -> usize {
        RawHeader::SIZE + self.nsymbt() + self.data_size()
    }
    
    /// Calculate data offset in file
    pub fn data_offset(&self) -> usize {
        RawHeader::SIZE + self.nsymbt()
    }
    
    /// Create a header builder
    pub fn builder() -> HeaderBuilder {
        HeaderBuilder::new()
    }
}

impl core::ops::Deref for Header {
    type Target = RawHeader;
    
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl core::ops::DerefMut for Header {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

/// Decode a value that was already loaded into memory via bytemuck
#[inline]
fn convert_i32_from_file_endian(value: i32, file_endian: FileEndian) -> i32 {
    if file_endian.is_native() {
        return value;
    }
    value.swap_bytes()
}

#[inline]
fn convert_f32_from_file_endian(value: f32, file_endian: FileEndian) -> f32 {
    if file_endian.is_native() {
        return value;
    }
    f32::from_bits(value.to_bits().swap_bytes())
}

impl TryFrom<RawHeader> for Header {
    type Error = Error;
    
    fn try_from(raw: RawHeader) -> Result<Self, Self::Error> {
        // Detect file endianness from MACHST, default to little-endian if unknown
        let (file_endian, detected) = FileEndian::from_machst_or_little(&raw.machst);
        
        // Convert dimensions
        let nx = convert_i32_from_file_endian(raw.nx, file_endian);
        let ny = convert_i32_from_file_endian(raw.ny, file_endian);
        let nz = convert_i32_from_file_endian(raw.nz, file_endian);
        
        // Validate dimensions
        if nx <= 0 || ny <= 0 || nz <= 0 {
            return Err(Error::InvalidDimensions);
        }
        
        // OOM protection
        let voxel_count = (nx as usize)
            .checked_mul(ny as usize)
            .and_then(|v| v.checked_mul(nz as usize))
            .ok_or(Error::InvalidDimensions)?;
        
        const MAX_REASONABLE_VOXELS: usize = 256 * 1024 * 1024 * 1024;
        if voxel_count > MAX_REASONABLE_VOXELS {
            return Err(Error::InvalidDimensions);
        }
        
        // Decode and validate mode
        let mode_val = convert_i32_from_file_endian(raw.mode, file_endian);
        let _mode = Mode::try_from(mode_val).map_err(|_| Error::InvalidMode)?;
        
        // Validate mode 12 requires f16 feature
        #[cfg(not(feature = "f16"))]
        if _mode == Mode::Float16 {
            return Err(Error::FeatureDisabled { feature: "f16" });
        }
        
        // Decode axis mapping
        let mapc = convert_i32_from_file_endian(raw.mapc, file_endian);
        let mapr = convert_i32_from_file_endian(raw.mapr, file_endian);
        let maps = convert_i32_from_file_endian(raw.maps, file_endian);
        let axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        
        // Decode nsymbt with OOM protection
        let nsymbt = convert_i32_from_file_endian(raw.nsymbt, file_endian);
        const MAX_EXTENDED_HEADER: usize = 1024 * 1024 * 1024;
        if nsymbt > MAX_EXTENDED_HEADER as i32 {
            return Err(Error::InvalidDimensions);
        }
        
        // Get nlabl with validation
        let nlabl = convert_i32_from_file_endian(raw.nlabl, file_endian);
        if !(0..=10).contains(&nlabl) {
            return Err(Error::InvalidHeader);
        }
        
        // Create header with converted values stored in raw
        let mut header = Self {
            raw,
            axis_map,
            file_endian,
            file_endian_detected: detected,
        };
        
        // Store converted values back to raw (now in native endian)
        header.raw.nx = nx;
        header.raw.ny = ny;
        header.raw.nz = nz;
        header.raw.mode = mode_val;
        header.raw.nxstart = convert_i32_from_file_endian(raw.nxstart, file_endian);
        header.raw.nystart = convert_i32_from_file_endian(raw.nystart, file_endian);
        header.raw.nzstart = convert_i32_from_file_endian(raw.nzstart, file_endian);
        header.raw.mx = convert_i32_from_file_endian(raw.mx, file_endian);
        header.raw.my = convert_i32_from_file_endian(raw.my, file_endian);
        header.raw.mz = convert_i32_from_file_endian(raw.mz, file_endian);
        header.raw.xlen = convert_f32_from_file_endian(raw.xlen, file_endian);
        header.raw.ylen = convert_f32_from_file_endian(raw.ylen, file_endian);
        header.raw.zlen = convert_f32_from_file_endian(raw.zlen, file_endian);
        header.raw.alpha = convert_f32_from_file_endian(raw.alpha, file_endian);
        header.raw.beta = convert_f32_from_file_endian(raw.beta, file_endian);
        header.raw.gamma = convert_f32_from_file_endian(raw.gamma, file_endian);
        header.raw.mapc = mapc;
        header.raw.mapr = mapr;
        header.raw.maps = maps;
        header.raw.dmin = convert_f32_from_file_endian(raw.dmin, file_endian);
        header.raw.dmax = convert_f32_from_file_endian(raw.dmax, file_endian);
        header.raw.dmean = convert_f32_from_file_endian(raw.dmean, file_endian);
        header.raw.ispg = convert_i32_from_file_endian(raw.ispg, file_endian);
        header.raw.nsymbt = nsymbt;
        header.raw.nlabl = nlabl;
        header.raw.origin[0] = convert_f32_from_file_endian(raw.origin[0], file_endian);
        header.raw.origin[1] = convert_f32_from_file_endian(raw.origin[1], file_endian);
        header.raw.origin[2] = convert_f32_from_file_endian(raw.origin[2], file_endian);
        header.raw.rms = convert_f32_from_file_endian(raw.rms, file_endian);
        
        Ok(header)
    }
}

/// Encode a value to file endianness
#[inline]
fn encode_i32_to_file_endian(value: i32, endian: FileEndian) -> i32 {
    if endian.is_native() {
        return value;
    }
    value.swap_bytes()
}

#[inline]
fn encode_f32_to_file_endian(value: f32, endian: FileEndian) -> f32 {
    if endian.is_native() {
        return value;
    }
    f32::from_bits(value.to_bits().swap_bytes())
}

impl From<Header> for RawHeader {
    fn from(header: Header) -> Self {
        let mut raw = header.raw;
        let endian = header.file_endian;
        
        // Encode all fields to file endianness
        raw.nx = encode_i32_to_file_endian(raw.nx, endian);
        raw.ny = encode_i32_to_file_endian(raw.ny, endian);
        raw.nz = encode_i32_to_file_endian(raw.nz, endian);
        raw.mode = encode_i32_to_file_endian(raw.mode, endian);
        raw.nxstart = encode_i32_to_file_endian(raw.nxstart, endian);
        raw.nystart = encode_i32_to_file_endian(raw.nystart, endian);
        raw.nzstart = encode_i32_to_file_endian(raw.nzstart, endian);
        raw.mx = encode_i32_to_file_endian(raw.mx, endian);
        raw.my = encode_i32_to_file_endian(raw.my, endian);
        raw.mz = encode_i32_to_file_endian(raw.mz, endian);
        raw.xlen = encode_f32_to_file_endian(raw.xlen, endian);
        raw.ylen = encode_f32_to_file_endian(raw.ylen, endian);
        raw.zlen = encode_f32_to_file_endian(raw.zlen, endian);
        raw.alpha = encode_f32_to_file_endian(raw.alpha, endian);
        raw.beta = encode_f32_to_file_endian(raw.beta, endian);
        raw.gamma = encode_f32_to_file_endian(raw.gamma, endian);
        raw.mapc = encode_i32_to_file_endian(header.axis_map.column as i32, endian);
        raw.mapr = encode_i32_to_file_endian(header.axis_map.row as i32, endian);
        raw.maps = encode_i32_to_file_endian(header.axis_map.section as i32, endian);
        raw.dmin = encode_f32_to_file_endian(raw.dmin, endian);
        raw.dmax = encode_f32_to_file_endian(raw.dmax, endian);
        raw.dmean = encode_f32_to_file_endian(raw.dmean, endian);
        raw.ispg = encode_i32_to_file_endian(raw.ispg, endian);
        raw.nsymbt = encode_i32_to_file_endian(raw.nsymbt, endian);
        raw.nlabl = encode_i32_to_file_endian(raw.nlabl, endian);
        raw.origin[0] = encode_f32_to_file_endian(raw.origin[0], endian);
        raw.origin[1] = encode_f32_to_file_endian(raw.origin[1], endian);
        raw.origin[2] = encode_f32_to_file_endian(raw.origin[2], endian);
        raw.rms = encode_f32_to_file_endian(raw.rms, endian);
        raw.machst = endian.to_machst();
        
        raw
    }
}

impl Default for Header {
    fn default() -> Self {
        let mut raw = RawHeader::new();
        // Set default dimensions to 1x1x1 (RawHeader::new() sets them to 0)
        raw.nx = 1;
        raw.ny = 1;
        raw.nz = 1;
        Self {
            raw,
            axis_map: AxisMap::default(),
            file_endian: FileEndian::native(),
            file_endian_detected: true,
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
        self.header.raw.alpha = alpha;
        self.header.raw.beta = beta;
        self.header.raw.gamma = gamma;
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
        self.header.raw.ispg = ispg;
        self
    }
    
    /// Set the extended header size in bytes
    pub fn extended_header_size(mut self, nsymbt: usize) -> Self {
        self.header.set_nsymbt(nsymbt);
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
