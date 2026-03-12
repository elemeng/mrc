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
    
    /// Set the EXTTYP identifier
    pub fn set_exttyp(&mut self, value: [u8; 4]) {
        self.exttyp = value;
    }
    
    /// Set EXTTYP from a 4-character string
    pub fn set_exttyp_str(&mut self, value: &str) -> Result<(), &'static str> {
        if value.len() != 4 {
            return Err("EXTTYP must be exactly 4 characters");
        }
        let bytes = value.as_bytes();
        self.exttyp.copy_from_slice(bytes);
        Ok(())
    }
    
    /// Set the NVERSION
    pub fn set_nversion(&mut self, value: i32) {
        self.nversion = value;
    }
    
    /// Set the axis map
    pub fn set_axis_map(&mut self, mapc: i32, mapr: i32, maps: i32) -> Result<(), Error> {
        self.axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        Ok(())
    }
    
    /// Set dimensions
    pub fn set_dimensions(&mut self, nx: usize, ny: usize, nz: usize) {
        self.nx = nx;
        self.ny = ny;
        self.nz = nz;
    }
    
    /// Set cell dimensions
    pub fn set_cell_dimensions(&mut self, xlen: f32, ylen: f32, zlen: f32) {
        self.xlen = xlen;
        self.ylen = ylen;
        self.zlen = zlen;
    }
    
    /// Set origin
    pub fn set_origin(&mut self, x: f32, y: f32, z: f32) {
        self.xorigin = x;
        self.yorigin = y;
        self.zorigin = z;
    }
    
    /// Set density statistics
    pub fn set_statistics(&mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) {
        self.dmin = dmin;
        self.dmax = dmax;
        self.dmean = dmean;
        self.rms = rms;
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
}

impl TryFrom<RawHeader> for Header {
    type Error = Error;
    
    fn try_from(raw: RawHeader) -> Result<Self, Self::Error> {
        // Detect file endianness from MACHST, default to little-endian if unknown
        let (file_endian, detected) = FileEndian::from_machst_or_little(&raw.machst);
        
        // Helper functions for endianness conversion
        let decode_i32 = |v: i32| -> i32 {
            match file_endian {
                FileEndian::Little => i32::from_le(v.to_le()),
                FileEndian::Big => i32::from_be(v.to_be()),
            }
        };
        
        let decode_f32 = |v: f32| -> f32 {
            let bits = v.to_bits();
            let converted = match file_endian {
                FileEndian::Little => u32::from_le(bits.to_le()),
                FileEndian::Big => u32::from_be(bits.to_be()),
            };
            f32::from_bits(converted)
        };
        
        // Decode dimensions with endianness conversion
        let nx = decode_i32(raw.nx);
        let ny = decode_i32(raw.ny);
        let nz = decode_i32(raw.nz);
        
        // Validate dimensions
        if nx <= 0 || ny <= 0 || nz <= 0 {
            return Err(Error::InvalidDimensions);
        }
        
        // Decode and validate mode
        let mode_val = decode_i32(raw.mode);
        let mode = Mode::try_from(mode_val).map_err(|_| Error::InvalidMode)?;
        
        // Validate mode 12 requires f16 feature
        #[cfg(not(feature = "f16"))]
        if mode == Mode::Float16 {
            return Err(Error::FeatureDisabled { feature: "f16" });
        }
        
        // Decode axis mapping with endianness conversion
        let mapc = decode_i32(raw.mapc);
        let mapr = decode_i32(raw.mapr);
        let maps = decode_i32(raw.maps);
        let axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        
        // Decode origin with endianness conversion
        let xorigin = decode_f32(raw.origin[0]);
        let yorigin = decode_f32(raw.origin[1]);
        let zorigin = decode_f32(raw.origin[2]);
        
        // Decode nversion from extra bytes
        let nversion = raw.nversion(file_endian);
        
        // Get nlabl with endianness conversion
        let nlabl = decode_i32(raw.nlabl);
        
        Ok(Self {
            nx: nx as usize,
            ny: ny as usize,
            nz: nz as usize,
            mode,
            nxstart: decode_i32(raw.nxstart),
            nystart: decode_i32(raw.nystart),
            nzstart: decode_i32(raw.nzstart),
            mx: decode_i32(raw.mx),
            my: decode_i32(raw.my),
            mz: decode_i32(raw.mz),
            xlen: decode_f32(raw.xlen),
            ylen: decode_f32(raw.ylen),
            zlen: decode_f32(raw.zlen),
            alpha: decode_f32(raw.alpha),
            beta: decode_f32(raw.beta),
            gamma: decode_f32(raw.gamma),
            axis_map,
            dmin: decode_f32(raw.dmin),
            dmax: decode_f32(raw.dmax),
            dmean: decode_f32(raw.dmean),
            ispg: decode_i32(raw.ispg),
            nsymbt: decode_i32(raw.nsymbt) as usize,
            exttyp: raw.exttyp(),
            nversion,
            file_endian,
            file_endian_detected: detected,
            xorigin,
            yorigin,
            zorigin,
            rms: decode_f32(raw.rms),
            nlabl,
            label: raw.label,
        })
    }
}

impl From<Header> for RawHeader {
    fn from(header: Header) -> Self {
        let mut raw = RawHeader::new();
        
        // Encode all fields with endianness conversion
        let encode_i32 = |v: i32, endian: FileEndian| -> i32 {
            match endian {
                FileEndian::Little => v.to_le(),
                FileEndian::Big => v.to_be(),
            }
        };
        
        let encode_f32 = |v: f32, endian: FileEndian| -> f32 {
            let bits = v.to_bits();
            let converted = match endian {
                FileEndian::Little => bits.to_le(),
                FileEndian::Big => bits.to_be(),
            };
            f32::from_bits(converted)
        };
        
        raw.nx = encode_i32(header.nx as i32, header.file_endian);
        raw.ny = encode_i32(header.ny as i32, header.file_endian);
        raw.nz = encode_i32(header.nz as i32, header.file_endian);
        raw.mode = encode_i32(header.mode.into(), header.file_endian);
        raw.nxstart = encode_i32(header.nxstart, header.file_endian);
        raw.nystart = encode_i32(header.nystart, header.file_endian);
        raw.nzstart = encode_i32(header.nzstart, header.file_endian);
        raw.mx = encode_i32(header.mx, header.file_endian);
        raw.my = encode_i32(header.my, header.file_endian);
        raw.mz = encode_i32(header.mz, header.file_endian);
        raw.xlen = encode_f32(header.xlen, header.file_endian);
        raw.ylen = encode_f32(header.ylen, header.file_endian);
        raw.zlen = encode_f32(header.zlen, header.file_endian);
        raw.alpha = encode_f32(header.alpha, header.file_endian);
        raw.beta = encode_f32(header.beta, header.file_endian);
        raw.gamma = encode_f32(header.gamma, header.file_endian);
        raw.mapc = encode_i32(header.axis_map.column as i32, header.file_endian);
        raw.mapr = encode_i32(header.axis_map.row as i32, header.file_endian);
        raw.maps = encode_i32(header.axis_map.section as i32, header.file_endian);
        raw.dmin = encode_f32(header.dmin, header.file_endian);
        raw.dmax = encode_f32(header.dmax, header.file_endian);
        raw.dmean = encode_f32(header.dmean, header.file_endian);
        raw.ispg = encode_i32(header.ispg, header.file_endian);
        raw.nsymbt = encode_i32(header.nsymbt as i32, header.file_endian);
        raw.set_exttyp(header.exttyp);
        raw.set_nversion(header.nversion, header.file_endian);
        raw.machst = header.file_endian.to_machst();
        raw.origin = [
            encode_f32(header.xorigin, header.file_endian),
            encode_f32(header.yorigin, header.file_endian),
            encode_f32(header.zorigin, header.file_endian),
        ];
        raw.rms = encode_f32(header.rms, header.file_endian);
        raw.nlabl = encode_i32(header.nlabl, header.file_endian);
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
