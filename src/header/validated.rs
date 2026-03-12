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
        let end = label_slice
            .iter()
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
        let dx = if nx > 0 {
            self.raw.xlen / nx as f32
        } else {
            0.0
        };
        let dy = if ny > 0 {
            self.raw.ylen / ny as f32
        } else {
            0.0
        };
        let dz = if nz > 0 {
            self.raw.zlen / nz as f32
        } else {
            0.0
        };
        (dx, dy, dz)
    }

    /// Calculate data size in bytes
    pub fn data_size(&self) -> usize {
        let voxel_count = self.voxel_count();
        let byte_size = self.mode().byte_size();
        if self.mode() == Mode::Packed4Bit {
            // Packed4Bit is special: 2 values per byte
            voxel_count.div_ceil(2)
        } else {
            voxel_count * byte_size
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

/// Trait for converting values between file and native endianness
trait EndianConvert: Copy {
    fn convert_from_file_endian(self, file_endian: FileEndian) -> Self;
    fn convert_to_file_endian(self, file_endian: FileEndian) -> Self;
}

impl EndianConvert for i32 {
    #[inline]
    fn convert_from_file_endian(self, file_endian: FileEndian) -> Self {
        if file_endian.is_native() {
            return self;
        }
        self.swap_bytes()
    }
    #[inline]
    fn convert_to_file_endian(self, file_endian: FileEndian) -> Self {
        if file_endian.is_native() {
            return self;
        }
        self.swap_bytes()
    }
}

impl EndianConvert for f32 {
    #[inline]
    fn convert_from_file_endian(self, file_endian: FileEndian) -> Self {
        if file_endian.is_native() {
            return self;
        }
        f32::from_bits(self.to_bits().swap_bytes())
    }
    #[inline]
    fn convert_to_file_endian(self, file_endian: FileEndian) -> Self {
        if file_endian.is_native() {
            return self;
        }
        f32::from_bits(self.to_bits().swap_bytes())
    }
}

impl TryFrom<RawHeader> for Header {
    type Error = Error;

    fn try_from(raw: RawHeader) -> Result<Self, Self::Error> {
        // Detect file endianness from MACHST, default to little-endian if unknown
        let (file_endian, detected) = FileEndian::from_machst_or_little(&raw.machst);

        // Convert dimensions
        let nx = raw.nx.convert_from_file_endian(file_endian);
        let ny = raw.ny.convert_from_file_endian(file_endian);
        let nz = raw.nz.convert_from_file_endian(file_endian);

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
        let mode_val = raw.mode.convert_from_file_endian(file_endian);
        let _mode = Mode::try_from(mode_val).map_err(|_| Error::InvalidMode)?;

        // Validate mode 12 requires f16 feature
        #[cfg(not(feature = "f16"))]
        if _mode == Mode::Float16 {
            return Err(Error::FeatureDisabled { feature: "f16" });
        }

        // Decode axis mapping
        let mapc = raw.mapc.convert_from_file_endian(file_endian);
        let mapr = raw.mapr.convert_from_file_endian(file_endian);
        let maps = raw.maps.convert_from_file_endian(file_endian);
        let axis_map = AxisMap::try_new(mapc, mapr, maps)?;

        // Decode nsymbt with OOM protection
        let nsymbt = raw.nsymbt.convert_from_file_endian(file_endian);
        const MAX_EXTENDED_HEADER: usize = 1024 * 1024 * 1024;
        if nsymbt > MAX_EXTENDED_HEADER as i32 {
            return Err(Error::InvalidDimensions);
        }

        // Get nlabl with validation
        let nlabl = raw.nlabl.convert_from_file_endian(file_endian);
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
        header.raw.nxstart = raw.nxstart.convert_from_file_endian(file_endian);
        header.raw.nystart = raw.nystart.convert_from_file_endian(file_endian);
        header.raw.nzstart = raw.nzstart.convert_from_file_endian(file_endian);
        header.raw.mx = raw.mx.convert_from_file_endian(file_endian);
        header.raw.my = raw.my.convert_from_file_endian(file_endian);
        header.raw.mz = raw.mz.convert_from_file_endian(file_endian);
        header.raw.xlen = raw.xlen.convert_from_file_endian(file_endian);
        header.raw.ylen = raw.ylen.convert_from_file_endian(file_endian);
        header.raw.zlen = raw.zlen.convert_from_file_endian(file_endian);
        header.raw.alpha = raw.alpha.convert_from_file_endian(file_endian);
        header.raw.beta = raw.beta.convert_from_file_endian(file_endian);
        header.raw.gamma = raw.gamma.convert_from_file_endian(file_endian);
        header.raw.mapc = mapc;
        header.raw.mapr = mapr;
        header.raw.maps = maps;
        header.raw.dmin = raw.dmin.convert_from_file_endian(file_endian);
        header.raw.dmax = raw.dmax.convert_from_file_endian(file_endian);
        header.raw.dmean = raw.dmean.convert_from_file_endian(file_endian);
        header.raw.ispg = raw.ispg.convert_from_file_endian(file_endian);
        header.raw.nsymbt = nsymbt;
        header.raw.nlabl = nlabl;
        header.raw.origin[0] = raw.origin[0].convert_from_file_endian(file_endian);
        header.raw.origin[1] = raw.origin[1].convert_from_file_endian(file_endian);
        header.raw.origin[2] = raw.origin[2].convert_from_file_endian(file_endian);
        header.raw.rms = raw.rms.convert_from_file_endian(file_endian);

        Ok(header)
    }
}



impl From<Header> for RawHeader {
    fn from(header: Header) -> Self {
        let mut raw = header.raw;
        let endian = header.file_endian;

        // Encode all fields to file endianness
        raw.nx = raw.nx.convert_to_file_endian(endian);
        raw.ny = raw.ny.convert_to_file_endian(endian);
        raw.nz = raw.nz.convert_to_file_endian(endian);
        raw.mode = raw.mode.convert_to_file_endian(endian);
        raw.nxstart = raw.nxstart.convert_to_file_endian(endian);
        raw.nystart = raw.nystart.convert_to_file_endian(endian);
        raw.nzstart = raw.nzstart.convert_to_file_endian(endian);
        raw.mx = raw.mx.convert_to_file_endian(endian);
        raw.my = raw.my.convert_to_file_endian(endian);
        raw.mz = raw.mz.convert_to_file_endian(endian);
        raw.xlen = raw.xlen.convert_to_file_endian(endian);
        raw.ylen = raw.ylen.convert_to_file_endian(endian);
        raw.zlen = raw.zlen.convert_to_file_endian(endian);
        raw.alpha = raw.alpha.convert_to_file_endian(endian);
        raw.beta = raw.beta.convert_to_file_endian(endian);
        raw.gamma = raw.gamma.convert_to_file_endian(endian);
        raw.mapc = (header.axis_map.column as i32).convert_to_file_endian(endian);
        raw.mapr = (header.axis_map.row as i32).convert_to_file_endian(endian);
        raw.maps = (header.axis_map.section as i32).convert_to_file_endian(endian);
        raw.dmin = raw.dmin.convert_to_file_endian(endian);
        raw.dmax = raw.dmax.convert_to_file_endian(endian);
        raw.dmean = raw.dmean.convert_to_file_endian(endian);
        raw.ispg = raw.ispg.convert_to_file_endian(endian);
        raw.nsymbt = raw.nsymbt.convert_to_file_endian(endian);
        raw.nlabl = raw.nlabl.convert_to_file_endian(endian);
        raw.origin[0] = raw.origin[0].convert_to_file_endian(endian);
        raw.origin[1] = raw.origin[1].convert_to_file_endian(endian);
        raw.origin[2] = raw.origin[2].convert_to_file_endian(endian);
        raw.rms = raw.rms.convert_to_file_endian(endian);
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
