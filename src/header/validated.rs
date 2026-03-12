//! Validated Header with semantic access and type conversions

use super::raw::RawHeader;
use crate::{AxisMap, Error, FileEndian, Mode};

/// Validated MRC Header with native-endian values
///
/// Created by validating a `RawHeader` and converting all fields
/// to their semantic types.
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
    
    // File Format
    /// File endianness
    pub file_endian: FileEndian,
    
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
}

impl Header {
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
}

impl TryFrom<RawHeader> for Header {
    type Error = Error;
    
    fn try_from(raw: RawHeader) -> Result<Self, Self::Error> {
        // Detect file endianness from MACHST
        let file_endian = FileEndian::from_machst(&raw.machst);
        
        // Validate dimensions
        if raw.nx <= 0 || raw.ny <= 0 || raw.nz <= 0 {
            return Err(Error::InvalidDimensions);
        }
        
        // Validate mode
        let mode = Mode::try_from(raw.mode).map_err(|_| Error::InvalidMode)?;
        
        // Validate mode 12 requires f16 feature
        #[cfg(not(feature = "f16"))]
        if mode == Mode::Float16 {
            return Err(Error::InvalidMode);
        }
        
        // Parse axis mapping
        let axis_map = AxisMap::try_new(raw.mapc, raw.mapr, raw.maps)?;
        
        // Get origin values
        let xorigin = raw.origin[0];
        let yorigin = raw.origin[1];
        let zorigin = raw.origin[2];
        
        Ok(Self {
            nx: raw.nx as usize,
            ny: raw.ny as usize,
            nz: raw.nz as usize,
            mode,
            nxstart: raw.nxstart,
            nystart: raw.nystart,
            nzstart: raw.nzstart,
            mx: raw.mx,
            my: raw.my,
            mz: raw.mz,
            xlen: raw.xlen,
            ylen: raw.ylen,
            zlen: raw.zlen,
            alpha: raw.alpha,
            beta: raw.beta,
            gamma: raw.gamma,
            axis_map,
            dmin: raw.dmin,
            dmax: raw.dmax,
            dmean: raw.dmean,
            ispg: raw.ispg,
            nsymbt: raw.nsymbt as usize,
            exttyp: raw.exttyp,
            file_endian,
            xorigin,
            yorigin,
            zorigin,
            rms: raw.rms,
        })
    }
}

impl From<Header> for RawHeader {
    fn from(header: Header) -> Self {
        let mut raw = RawHeader::new();
        
        raw.nx = header.nx as i32;
        raw.ny = header.ny as i32;
        raw.nz = header.nz as i32;
        raw.mode = header.mode.into();
        raw.nxstart = header.nxstart;
        raw.nystart = header.nystart;
        raw.nzstart = header.nzstart;
        raw.mx = header.mx;
        raw.my = header.my;
        raw.mz = header.mz;
        raw.xlen = header.xlen;
        raw.ylen = header.ylen;
        raw.zlen = header.zlen;
        raw.alpha = header.alpha;
        raw.beta = header.beta;
        raw.gamma = header.gamma;
        raw.mapc = header.axis_map.column as i32;
        raw.mapr = header.axis_map.row as i32;
        raw.maps = header.axis_map.section as i32;
        raw.dmin = header.dmin;
        raw.dmax = header.dmax;
        raw.dmean = header.dmean;
        raw.ispg = header.ispg;
        raw.nsymbt = header.nsymbt as i32;
        raw.exttyp = header.exttyp;
        raw.machst = header.file_endian.to_machst();
        raw.origin = [header.xorigin, header.yorigin, header.zorigin];
        raw.rms = header.rms;
        
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
            file_endian: FileEndian::native(),
            xorigin: 0.0,
            yorigin: 0.0,
            zorigin: 0.0,
            rms: 0.0,
        }
    }
}