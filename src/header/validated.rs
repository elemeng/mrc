//! MRC Header - single unified type with validated, native-endian values

use super::raw::RawHeader;
use crate::core::{AxisMap, Error, Mode};
use crate::voxel::{EndianConvert, FileEndian};

/// MRC Header with validated, native-endian values
///
/// This is the single header type for all operations. It provides:
/// - Validated dimensions and mode
/// - All values in native endianness
/// - Clean accessor methods
/// - Builder pattern for construction
#[derive(Debug, Clone)]
pub struct Header {
    // Dimensions (validated, always positive)
    nx: usize,
    ny: usize,
    nz: usize,

    // Data mode
    mode: Mode,

    // Start positions (for FFT data)
    nxstart: i32,
    nystart: i32,
    nzstart: i32,

    // Grid sampling
    mx: i32,
    my: i32,
    mz: i32,

    // Cell dimensions (Angstroms)
    xlen: f32,
    ylen: f32,
    zlen: f32,

    // Cell angles (degrees)
    alpha: f32,
    beta: f32,
    gamma: f32,

    // Axis mapping
    axis_map: AxisMap,

    // Statistics
    dmin: f32,
    dmax: f32,
    dmean: f32,
    rms: f32,

    // Space group
    ispg: i32,

    // Extended header size
    nsymbt: usize,

    // Extended header type (EXTTYP)
    exttyp: [u8; 4],

    // Format version (NVERSION)
    nversion: i32,

    // Origin
    origin: [f32; 3],

    // File endianness (internal)
    file_endian: FileEndian,

    // Labels
    labels: [Option<alloc::string::String>; 10],
    nlabl: usize,
}

impl Header {
    /// Header size in bytes
    pub const SIZE: usize = 1024;

    /// Create a new header with default values
    pub fn new() -> Self {
        Self {
            nx: 1,
            ny: 1,
            nz: 1,
            mode: Mode::Float32,
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
            axis_map: AxisMap::default(),
            dmin: f32::INFINITY,
            dmax: f32::NEG_INFINITY,
            dmean: f32::NEG_INFINITY,
            rms: -1.0,
            ispg: 1,
            nsymbt: 0,
            exttyp: [0; 4],
            nversion: 20140,
            origin: [0.0; 3],
            file_endian: FileEndian::native(),
            labels: Default::default(),
            nlabl: 0,
        }
    }

    // === Dimension accessors ===

    #[inline]
    pub fn nx(&self) -> usize {
        self.nx
    }

    #[inline]
    pub fn ny(&self) -> usize {
        self.ny
    }

    #[inline]
    pub fn nz(&self) -> usize {
        self.nz
    }

    #[inline]
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.nx, self.ny, self.nz)
    }

    /// Set dimensions
    pub fn set_dimensions(&mut self, nx: usize, ny: usize, nz: usize) -> &mut Self {
        self.nx = nx;
        self.ny = ny;
        self.nz = nz;
        self
    }

    // === Mode accessor ===

    #[inline]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) -> &mut Self {
        self.mode = mode;
        self
    }

    // === Origin accessors ===

    #[inline]
    pub fn xorigin(&self) -> f32 {
        self.origin[0]
    }

    #[inline]
    pub fn yorigin(&self) -> f32 {
        self.origin[1]
    }

    #[inline]
    pub fn zorigin(&self) -> f32 {
        self.origin[2]
    }

    #[inline]
    pub fn origin(&self) -> [f32; 3] {
        self.origin
    }

    pub fn set_origin(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.origin = [x, y, z];
        self
    }

    // === Cell dimensions ===

    #[inline]
    pub fn xlen(&self) -> f32 {
        self.xlen
    }

    #[inline]
    pub fn ylen(&self) -> f32 {
        self.ylen
    }

    #[inline]
    pub fn zlen(&self) -> f32 {
        self.zlen
    }

    pub fn set_cell_dimensions(&mut self, xlen: f32, ylen: f32, zlen: f32) -> &mut Self {
        self.xlen = xlen;
        self.ylen = ylen;
        self.zlen = zlen;
        self
    }

    /// Get voxel size in Angstroms
    pub fn voxel_size(&self) -> (f32, f32, f32) {
        let dx = if self.nx > 0 {
            self.xlen / self.nx as f32
        } else {
            0.0
        };
        let dy = if self.ny > 0 {
            self.ylen / self.ny as f32
        } else {
            0.0
        };
        let dz = if self.nz > 0 {
            self.zlen / self.nz as f32
        } else {
            0.0
        };
        (dx, dy, dz)
    }

    // === Cell angles ===

    pub fn set_cell_angles(&mut self, alpha: f32, beta: f32, gamma: f32) -> &mut Self {
        self.alpha = alpha;
        self.beta = beta;
        self.gamma = gamma;
        self
    }

    // === Statistics ===

    #[inline]
    pub fn dmin(&self) -> f32 {
        self.dmin
    }

    #[inline]
    pub fn dmax(&self) -> f32 {
        self.dmax
    }

    #[inline]
    pub fn dmean(&self) -> f32 {
        self.dmean
    }

    #[inline]
    pub fn rms(&self) -> f32 {
        self.rms
    }

    pub fn set_statistics(&mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) -> &mut Self {
        self.dmin = dmin;
        self.dmax = dmax;
        self.dmean = dmean;
        self.rms = rms;
        self
    }

    // === Axis mapping ===

    #[inline]
    pub fn axis_map(&self) -> &AxisMap {
        &self.axis_map
    }

    pub fn set_axis_map(&mut self, mapc: i32, mapr: i32, maps: i32) -> Result<&mut Self, Error> {
        self.axis_map = AxisMap::try_new(mapc, mapr, maps)?;
        Ok(self)
    }

    // === Extended header ===

    #[inline]
    pub fn nsymbt(&self) -> usize {
        self.nsymbt
    }

    pub fn set_nsymbt(&mut self, nsymbt: usize) -> &mut Self {
        self.nsymbt = nsymbt;
        self
    }

    #[inline]
    pub fn exttyp(&self) -> [u8; 4] {
        self.exttyp
    }

    pub fn set_exttyp(&mut self, value: [u8; 4]) -> &mut Self {
        self.exttyp = value;
        self
    }

    pub fn set_exttyp_str(&mut self, value: &str) -> Result<&mut Self, Error> {
        if value.len() != 4 {
            return Err(Error::InvalidHeader);
        }
        self.exttyp.copy_from_slice(value.as_bytes());
        Ok(self)
    }

    #[inline]
    pub fn nversion(&self) -> i32 {
        self.nversion
    }

    pub fn set_nversion(&mut self, value: i32) -> &mut Self {
        self.nversion = value;
        self
    }

    // === Space group ===

    #[inline]
    pub fn ispg(&self) -> i32 {
        self.ispg
    }

    pub fn set_space_group(&mut self, ispg: i32) -> &mut Self {
        self.ispg = ispg;
        self
    }

    // === Labels ===

    #[inline]
    pub fn nlabl(&self) -> usize {
        self.nlabl
    }

    pub fn label(&self, index: usize) -> Option<&str> {
        self.labels.get(index)?.as_ref().map(|s| s.as_str())
    }

    pub fn set_label(&mut self, index: usize, text: &str) -> Result<&mut Self, Error> {
        if index >= 10 {
            return Err(Error::InvalidHeader);
        }
        let truncated: alloc::string::String = text.chars().take(80).collect();
        self.labels[index] = Some(truncated);
        if index >= self.nlabl {
            self.nlabl = index + 1;
        }
        Ok(self)
    }

    // === Endianness (internal) ===

    #[inline]
    pub(crate) fn file_endian(&self) -> FileEndian {
        self.file_endian
    }

    /// Returns true if file uses native endianness (zero-copy safe)
    #[inline]
    pub fn is_native_endian(&self) -> bool {
        self.file_endian.is_native()
    }

    // === Computed properties ===

    /// Total number of voxels
    #[inline]
    pub fn voxel_count(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Calculate data size in bytes
    pub fn data_size(&self) -> usize {
        let voxel_count = self.voxel_count();
        if self.mode == Mode::Packed4Bit {
            voxel_count.div_ceil(2)
        } else {
            voxel_count * self.mode.byte_size()
        }
    }

    /// Calculate total file size
    pub fn file_size(&self) -> usize {
        Self::SIZE + self.nsymbt + self.data_size()
    }

    /// Calculate data offset in file
    pub fn data_offset(&self) -> usize {
        Self::SIZE + self.nsymbt
    }

    // === Binary I/O ===

    /// Parse header from 1024-byte buffer
    pub fn from_bytes(bytes: &[u8; 1024]) -> Result<Self, Error> {
        let raw: &RawHeader = bytemuck::from_bytes(bytes);

        // Detect endianness
        let (file_endian, _detected) = FileEndian::from_machst_or_little(&raw.machst);

        // Convert and validate dimensions
        let nx = raw.nx.convert_from_file(file_endian);
        let ny = raw.ny.convert_from_file(file_endian);
        let nz = raw.nz.convert_from_file(file_endian);

        if nx <= 0 || ny <= 0 || nz <= 0 {
            return Err(Error::InvalidDimensions);
        }

        // OOM protection
        let voxel_count = (nx as usize)
            .checked_mul(ny as usize)
            .and_then(|v| v.checked_mul(nz as usize))
            .ok_or(Error::InvalidDimensions)?;

        const MAX_VOXELS: usize = 256 * 1024 * 1024 * 1024;
        if voxel_count > MAX_VOXELS {
            return Err(Error::InvalidDimensions);
        }

        // Validate mode
        let mode_val = raw.mode.convert_from_file(file_endian);
        let mode = Mode::try_from(mode_val).map_err(|_| Error::InvalidMode)?;

        #[cfg(not(feature = "f16"))]
        if mode == Mode::Float16 {
            return Err(Error::FeatureDisabled { feature: "f16" });
        }

        // Validate axis map
        let mapc = raw.mapc.convert_from_file(file_endian);
        let mapr = raw.mapr.convert_from_file(file_endian);
        let maps = raw.maps.convert_from_file(file_endian);
        let axis_map = AxisMap::try_new(mapc, mapr, maps)?;

        // Validate nsymbt
        let nsymbt = raw.nsymbt.convert_from_file(file_endian);
        if nsymbt < 0 || nsymbt as usize > 1024 * 1024 * 1024 {
            return Err(Error::InvalidHeader);
        }

        // Validate nlabl
        let nlabl = raw.nlabl.convert_from_file(file_endian);
        if !(0..=10).contains(&nlabl) {
            return Err(Error::InvalidHeader);
        }

        // Extract labels
        let mut labels: [Option<alloc::string::String>; 10] = Default::default();
        for (i, label_slot) in labels.iter_mut().enumerate().take(nlabl as usize) {
            let start = i * 80;
            let slice = &raw.label[start..start + 80];
            let end = slice
                .iter()
                .rposition(|&b| b != b' ' && b != 0)
                .map(|pos| pos + 1)
                .unwrap_or(0);
            if end > 0 {
                *label_slot = alloc::string::String::from_utf8(slice[..end].to_vec()).ok();
            }
        }

        Ok(Self {
            nx: nx as usize,
            ny: ny as usize,
            nz: nz as usize,
            mode,
            nxstart: raw.nxstart.convert_from_file(file_endian),
            nystart: raw.nystart.convert_from_file(file_endian),
            nzstart: raw.nzstart.convert_from_file(file_endian),
            mx: raw.mx.convert_from_file(file_endian),
            my: raw.my.convert_from_file(file_endian),
            mz: raw.mz.convert_from_file(file_endian),
            xlen: raw.xlen.convert_from_file(file_endian),
            ylen: raw.ylen.convert_from_file(file_endian),
            zlen: raw.zlen.convert_from_file(file_endian),
            alpha: raw.alpha.convert_from_file(file_endian),
            beta: raw.beta.convert_from_file(file_endian),
            gamma: raw.gamma.convert_from_file(file_endian),
            axis_map,
            dmin: raw.dmin.convert_from_file(file_endian),
            dmax: raw.dmax.convert_from_file(file_endian),
            dmean: raw.dmean.convert_from_file(file_endian),
            rms: raw.rms.convert_from_file(file_endian),
            ispg: raw.ispg.convert_from_file(file_endian),
            nsymbt: nsymbt as usize,
            exttyp: raw.exttyp(),
            nversion: raw.nversion(file_endian),
            origin: [
                raw.origin[0].convert_from_file(file_endian),
                raw.origin[1].convert_from_file(file_endian),
                raw.origin[2].convert_from_file(file_endian),
            ],
            file_endian,
            labels,
            nlabl: nlabl as usize,
        })
    }

    /// Serialize header to 1024-byte buffer
    pub fn to_bytes(&self) -> [u8; 1024] {
        let mut raw = RawHeader::new();

        macro_rules! set_field {
            ($field:ident, $value:expr) => {
                raw.$field = $value.convert_from_file(self.file_endian);
            };
        }

        raw.nx = (self.nx as i32).convert_from_file(self.file_endian);
        raw.ny = (self.ny as i32).convert_from_file(self.file_endian);
        raw.nz = (self.nz as i32).convert_from_file(self.file_endian);
        raw.mode = (self.mode as i32).convert_from_file(self.file_endian);

        set_field!(nxstart, self.nxstart);
        set_field!(nystart, self.nystart);
        set_field!(nzstart, self.nzstart);
        set_field!(mx, self.mx);
        set_field!(my, self.my);
        set_field!(mz, self.mz);
        set_field!(xlen, self.xlen);
        set_field!(ylen, self.ylen);
        set_field!(zlen, self.zlen);
        set_field!(alpha, self.alpha);
        set_field!(beta, self.beta);
        set_field!(gamma, self.gamma);

        raw.mapc = (self.axis_map.column as i32).convert_from_file(self.file_endian);
        raw.mapr = (self.axis_map.row as i32).convert_from_file(self.file_endian);
        raw.maps = (self.axis_map.section as i32).convert_from_file(self.file_endian);

        set_field!(dmin, self.dmin);
        set_field!(dmax, self.dmax);
        set_field!(dmean, self.dmean);
        set_field!(rms, self.rms);
        set_field!(ispg, self.ispg);
        raw.nsymbt = (self.nsymbt as i32).convert_from_file(self.file_endian);

        raw.set_exttyp(self.exttyp);
        raw.set_nversion(self.nversion, self.file_endian);

        raw.origin[0] = self.origin[0].convert_from_file(self.file_endian);
        raw.origin[1] = self.origin[1].convert_from_file(self.file_endian);
        raw.origin[2] = self.origin[2].convert_from_file(self.file_endian);

        raw.machst = self.file_endian.to_machst();
        raw.nlabl = (self.nlabl as i32).convert_from_file(self.file_endian);

        // Write labels
        for i in 0..self.nlabl {
            let start = i * 80;
            if let Some(ref label) = self.labels[i] {
                let bytes = label.as_bytes();
                let len = bytes.len().min(80);
                raw.label[start..start + len].copy_from_slice(&bytes[..len]);
            }
        }

        let bytes: &[u8; 1024] = bytemuck::from_bytes(bytemuck::bytes_of(&raw));
        *bytes
    }

    /// Create a header builder
    pub fn builder() -> HeaderBuilder {
        HeaderBuilder::new()
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing MRC headers
#[derive(Debug)]
pub struct HeaderBuilder {
    header: Header,
}

impl HeaderBuilder {
    pub fn new() -> Self {
        Self {
            header: Header::new(),
        }
    }

    pub fn dimensions(mut self, nx: usize, ny: usize, nz: usize) -> Self {
        self.header.set_dimensions(nx, ny, nz);
        self
    }

    pub fn mode(mut self, mode: Mode) -> Self {
        self.header.set_mode(mode);
        self
    }

    pub fn voxel_size(mut self, dx: f32, dy: f32, dz: f32) -> Self {
        let (nx, ny, nz) = self.header.dimensions();
        self.header
            .set_cell_dimensions(dx * nx as f32, dy * ny as f32, dz * nz as f32);
        self
    }

    pub fn origin(mut self, x: f32, y: f32, z: f32) -> Self {
        self.header.set_origin(x, y, z);
        self
    }

    pub fn cell_dimensions(mut self, xlen: f32, ylen: f32, zlen: f32) -> Self {
        self.header.set_cell_dimensions(xlen, ylen, zlen);
        self
    }

    pub fn cell_angles(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
        self.header.set_cell_angles(alpha, beta, gamma);
        self
    }

    pub fn axis_map(mut self, mapc: i32, mapr: i32, maps: i32) -> Self {
        let _ = self.header.set_axis_map(mapc, mapr, maps);
        self
    }

    pub fn statistics(mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) -> Self {
        self.header.set_statistics(dmin, dmax, dmean, rms);
        self
    }

    pub fn space_group(mut self, ispg: i32) -> Self {
        self.header.set_space_group(ispg);
        self
    }

    pub fn extended_header_size(mut self, nsymbt: usize) -> Self {
        self.header.set_nsymbt(nsymbt);
        self
    }

    pub fn exttyp(mut self, exttyp: [u8; 4]) -> Self {
        self.header.set_exttyp(exttyp);
        self
    }

    pub fn label(mut self, index: usize, text: &str) -> Self {
        let _ = self.header.set_label(index, text);
        self
    }

    pub fn build(self) -> Header {
        self.header
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

    #[test]
    fn test_header_default() {
        let header = Header::default();
        assert_eq!(header.dimensions(), (1, 1, 1));
        assert_eq!(header.mode(), Mode::Float32);
    }

    #[test]
    fn test_header_builder() {
        let header = Header::builder()
            .dimensions(64, 64, 64)
            .mode(Mode::Int16)
            .origin(10.0, 20.0, 30.0)
            .build();

        assert_eq!(header.dimensions(), (64, 64, 64));
        assert_eq!(header.mode(), Mode::Int16);
        assert_eq!(header.xorigin(), 10.0);
    }

    #[test]
    fn test_roundtrip() {
        let original = Header::builder()
            .dimensions(128, 128, 64)
            .mode(Mode::Float32)
            .origin(5.0, 10.0, 15.0)
            .statistics(0.0, 100.0, 50.0, 25.0)
            .build();

        let bytes = original.to_bytes();
        let parsed = Header::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.dimensions(), original.dimensions());
        assert_eq!(parsed.mode(), original.mode());
        assert_eq!(parsed.xorigin(), original.xorigin());
    }
}
