//! Consolidated MRC file reader with automatic backend selection.
//!
//! Provides [`Reader`], which auto-selects between memory-mapped (zero-copy)
//! and buffered I/O based on the file and platform capabilities. Use
//! [`Reader::open`] for files or [`Reader::from_reader`] for custom sources.
//!
//! Also provides compression detection helpers used by the reader constructors.

use crate::VoxelBlock;
use crate::engine::block::VolumeShape;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::borrow::Cow;
use std::path::Path;
use std::vec::Vec;

// ============================================================================
// Compression detection (used internally by Reader::open)
// ============================================================================

/// Detected compression format of an MRC file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompressionType {
    /// Uncompressed MRC file.
    Plain,
    /// Gzip-compressed MRC file.
    #[cfg(feature = "gzip")]
    Gzip,
    /// Bzip2-compressed MRC file.
    #[cfg(feature = "bzip2")]
    Bzip2,
}

/// Peek at a byte slice to determine its compression format.
#[doc(hidden)]
pub fn detect_compression_from_bytes(bytes: &[u8]) -> CompressionType {
    if bytes.len() < 2 {
        return CompressionType::Plain;
    }
    let magic = [bytes[0], bytes[1]];
    #[cfg(feature = "gzip")]
    if magic == [0x1f, 0x8b] {
        return CompressionType::Gzip;
    }
    #[cfg(feature = "bzip2")]
    if magic == [b'B', b'Z'] {
        return CompressionType::Bzip2;
    }
    CompressionType::Plain
}

/// Peek at the first bytes of a file to determine its compression format.
pub fn detect_compression<P: AsRef<Path>>(path: P) -> Result<CompressionType, Error> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut buf = [0u8; 2];
    let n = file.read(&mut buf)?;
    Ok(detect_compression_from_bytes(&buf[..n]))
}

// ============================================================================
// ============================================================================
// Data source and Reader type
// ============================================================================

/// How the reader accesses voxel data.
#[derive(Debug)]
enum DataSource {
    /// Loaded entirely into memory.
    Buffered { data: Vec<u8>, truncated: bool },
    /// Memory-mapped file (zero-copy).
    #[cfg(feature = "mmap")]
    Mmap {
        map: memmap2::Mmap,
        data_offset: usize,
        truncated: bool,
    },
}

/// MRC file reader with automatic backend selection.
///
/// Opens files via memory mapping (zero-copy for large files) or buffered
/// I/O (in-memory for smaller files). Accepts custom [`std::io::Read`] sources
/// via [`from_reader`](Self::from_reader).
///
/// All iteration methods (`slices`, `slabs`, `tiles`, `subregion`, etc.)
/// are **inherent methods** — no trait imports needed.
///
/// # Zero-copy access
///
/// When backed by a memory map (which [`open`](Self::open) selects
/// automatically for files), [`slab_as`](Self::slab_as) returns `&[T]`
/// directly into the mapped memory with no allocation. The same zero-copy
/// access is also available for buffered readers (created via
/// [`from_bytes`](Self::from_bytes), [`from_reader`](Self::from_reader),
/// or compressed-file constructors) under the same conditions.
/// Requires native endianness and matching voxel type.
///
/// # Example
/// ```no_run
/// use mrc::Reader;
///
/// let reader = Reader::open("density.mrc")?;
/// for slice in reader.slices::<f32>() {
///     let block = slice?;
/// }
/// # Ok::<_, mrc::Error>(())
/// ```
#[derive(Debug)]
pub struct Reader {
    pub(crate) header: Header,
    pub(crate) ext_header: Vec<u8>,
    pub(crate) endian: FileEndian,
    pub(crate) mode: Mode,
    pub(crate) shape: VolumeShape,
    source: DataSource,
}

// ============================================================================
// Constructors
// ============================================================================

impl Reader {
    /// Open an MRC file, auto-detecting gzip/bzip2 compression.
    ///
    /// For plain files, selects memory-mapped I/O when available (the `mmap`
    /// feature) and falls back to buffered I/O otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let reader = mrc::Reader::open("density.mrc")?;
    /// println!("Dimensions: {}x{}x{}", reader.shape().nx, reader.shape().ny, reader.shape().nz);
    /// # Ok(())
    /// # }
    /// ```
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Self::_open_detect(path.as_ref(), false).map(|(r, _)| r)
    }

    /// Open in **permissive** mode.
    ///
    /// Non-fatal header issues are collected as warnings instead of errors.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let (reader, warnings) = mrc::Reader::open_permissive("density.mrc")?;
    /// for w in &warnings {
    ///     eprintln!("Warning: {w}");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn open_permissive<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_detect(path.as_ref(), true)
    }

    /// Open a plain (uncompressed) MRC file via buffered I/O.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let reader = mrc::Reader::open_plain("density.mrc")?;
    /// println!("Shape: {:?}", reader.shape());
    /// # Ok(())
    /// # }
    /// ```
    pub fn open_plain<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Self::_open_plain(path, false).map(|(r, _)| r)
    }

    /// Read an MRC file from any [`std::io::Read`] source.
    ///
    /// The entire source is read into memory, then parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// use std::io::Cursor;
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![0u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// let reader = mrc::Reader::from_reader(Cursor::new(buf))?;
    /// assert_eq!(reader.shape().nx, 4);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_reader<R: std::io::Read>(mut reader: R) -> Result<Self, Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Self::_read_from_buf(buf, false).map(|(r, _)| r)
    }

    /// Read from any [`std::io::Read`] source in permissive mode.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// use std::io::Cursor;
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![0u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// let (reader, warnings) = mrc::Reader::from_reader_permissive(Cursor::new(buf))?;
    /// assert!(warnings.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_reader_permissive<R: std::io::Read>(
        mut reader: R,
    ) -> Result<(Self, Vec<String>), Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Self::_read_from_buf(buf, true)
    }

    /// Parse an MRC file from an in-memory byte buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![0u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// let reader = mrc::Reader::from_bytes(buf)?;
    /// assert_eq!(reader.shape().nx, 4);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, Error> {
        Self::_read_from_buf(data, false).map(|(r, _)| r)
    }

    /// Parse an MRC file from an in-memory byte buffer in permissive mode.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![0u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// let (reader, warnings) = mrc::Reader::from_bytes_permissive(buf)?;
    /// assert!(warnings.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_bytes_permissive(data: Vec<u8>) -> Result<(Self, Vec<String>), Error> {
        Self::_read_from_buf(data, true)
    }

    // ── Internal open helpers ──────────────────────────────────────────

    /// Detect compression and open. Tries mmap first for plain files.
    fn _open_detect(
        path: &std::path::Path,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        use std::io::{Read, Seek};

        let mut file = std::fs::File::open(path)?;
        let mut magic = [0u8; 2];
        let n = file.read(&mut magic)?;

        if n >= 2 {
            match magic {
                #[cfg(feature = "gzip")]
                [0x1f, 0x8b] => {
                    // Seek back to start before handing to the gzip decoder.
                    // An error here is benign — the decoder will fail on its own.
                    let _ = file.seek(std::io::SeekFrom::Start(0));
                    return Self::_open_gzip_file(
                        file,
                        permissive,
                        crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
                    );
                }
                #[cfg(feature = "bzip2")]
                [b'B', b'Z'] => {
                    // Seek back to start before handing to the bzip2 decoder.
                    // An error here is benign — the decoder will fail on its own.
                    let _ = file.seek(std::io::SeekFrom::Start(0));
                    return Self::_open_bzip2_file(
                        file,
                        permissive,
                        crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
                    );
                }
                _ => {}
            }
        }

        // Plain file — try mmap first; fall back to buffered on any error.
        #[cfg(feature = "mmap")]
        {
            drop(file);
            if let Ok(result) = Self::_open_mmap_path(path, permissive) {
                return Ok(result);
            }
            // mmap failed — re-open for buffered fallback.
            let file = std::fs::File::open(path)?;
            Self::_open_plain_file(file, permissive)
        }

        #[cfg(not(feature = "mmap"))]
        {
            // Seek back to start (file is at offset 2 after reading magic bytes).
            // An error here is benign — the plain-file reader will fail with
            // its own I/O error if the file is genuinely unreadable.
            let _ = file.seek(std::io::SeekFrom::Start(0));
            Self::_open_plain_file(file, permissive)
        }
    }

    fn _open_plain<P: AsRef<std::path::Path>>(
        path: P,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_plain_file(std::fs::File::open(path)?, permissive)
    }

    fn _open_plain_file(
        mut file: std::fs::File,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        use std::io::Read;

        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let (header, warnings, _endian, data_size) =
            crate::io::reader_common::parse_header(&header_bytes, permissive)?;

        let ext_size = header.nsymbt as usize;
        let mut ext_header = vec![0u8; ext_size];
        if ext_size > 0 {
            file.read_exact(&mut ext_header)?;
        }

        let mut data = vec![0u8; data_size];
        file.read_exact(&mut data)?;

        if !permissive {
            let file_len = file.metadata()?.len() as usize;
            let expected_len = header.data_offset() + data_size;
            if file_len != expected_len {
                return Err(Error::FileSizeMismatch {
                    expected: expected_len,
                    actual: file_len,
                });
            }
        }

        Self::_build(
            header,
            ext_header,
            DataSource::Buffered {
                data,
                truncated: false,
            },
            warnings,
        )
    }

    fn _read_from_buf(data: Vec<u8>, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        if data.len() < 1024 {
            return Err(Error::InvalidHeader);
        }
        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&data[..1024]);
        let (header, mut warnings, _endian, data_size) =
            crate::io::reader_common::parse_header(&header_bytes, permissive)?;

        let ext_size = header.nsymbt as usize;
        let ext_end = (1024 + ext_size).min(data.len());
        let ext_header = if ext_size > 0 && ext_end > 1024 {
            if ext_end < 1024 + ext_size {
                warnings.push(format!(
                    "Extended header truncated: expected {} bytes, got {}",
                    ext_size,
                    ext_end - 1024
                ));
            }
            data[1024..ext_end].to_vec()
        } else {
            Vec::new()
        };

        let data_offset = header.data_offset();
        let voxel_data = if data_offset < data.len() {
            let available = data.len() - data_offset;
            let expected = data_size.min(available);
            data[data_offset..data_offset + expected].to_vec()
        } else {
            Vec::new()
        };

        if !permissive && voxel_data.len() != data_size {
            return Err(Error::FileSizeMismatch {
                expected: header.data_offset() + data_size,
                actual: data.len(),
            });
        }

        let truncated = voxel_data.len() != data_size;
        Self::_build(
            header,
            ext_header,
            DataSource::Buffered {
                data: voxel_data,
                truncated,
            },
            warnings,
        )
    }

    #[cfg(feature = "mmap")]
    fn _open_mmap_path(
        path: &std::path::Path,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        use std::fs::File;

        let file = File::open(path)?;
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&file)
                .map_err(|_| Error::Mmap)?
        };
        // File is closed here; mmap keeps the mapping alive.

        // Read header from mmap (file is already mapped)
        if mmap.len() < 1024 {
            return Err(Error::InvalidHeader);
        }
        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&mmap[..1024]);

        let (header, warnings, _endian, data_size) =
            crate::io::reader_common::parse_header(&header_bytes, permissive)?;

        let expected_size = header
            .data_offset()
            .checked_add(data_size)
            .ok_or(Error::InvalidHeader)?;
        let truncated = if !permissive {
            if mmap.len() != expected_size {
                return Err(Error::FileSizeMismatch {
                    expected: expected_size,
                    actual: mmap.len(),
                });
            }
            false
        } else if mmap.len() < header.data_offset() {
            return Err(Error::FileSizeMismatch {
                expected: header.data_offset(),
                actual: mmap.len(),
            });
        } else {
            mmap.len() < expected_size
        };

        // IMOD detection is done in _build; warnings passed through
        Self::_build(
            header,
            Vec::new(), // ext_header read from mmap on demand
            DataSource::Mmap {
                map: mmap,
                data_offset: header.data_offset(),
                truncated,
            },
            warnings,
        )
    }

    /// Common path: build a Reader from parsed header + data source.
    fn _build(
        header: Header,
        ext_header: Vec<u8>,
        source: DataSource,
        warnings: Vec<String>,
    ) -> Result<(Self, Vec<String>), Error> {
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        let endian = header.detect_endian();

        let mut warnings = warnings;
        if mode == Mode::Int8 {
            if let Some(imod) = header.detect_imod() {
                if !imod.bytes_are_signed {
                    warnings.push(
                        "IMOD file with unsigned Mode 0 detected: use slices_mode0() \
                         or convert::<f32>() for correct values"
                            .into(),
                    );
                }
            }
        }

        Ok((
            Self {
                header,
                ext_header,
                endian,
                mode,
                shape,
                source,
            },
            warnings,
        ))
    }

    /// Construct a Reader from a decompressed MRC (used by gzip/bzip2 readers).
    pub(crate) fn _from_decompressed(
        d: crate::io::reader_common::DecompressedMrc,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_build(
            d.header,
            d.ext_header,
            DataSource::Buffered {
                data: d.data,
                truncated: false,
            },
            d.warnings,
        )
    }
}

// ============================================================================
// Public accessors
// ============================================================================

impl Reader {
    /// Volume dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let s = reader.shape();
    /// assert_eq!(s.nx, 4);
    /// # Ok(())
    /// # }
    /// ```
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # h.mode = 0;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 16]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert_eq!(reader.mode(), mrc::Mode::Int8);
    /// # Ok(())
    /// # }
    /// ```
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// A reference to the parsed header.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let header = reader.header();
    /// assert_eq!(header.nx, 4);
    /// # Ok(())
    /// # }
    /// ```
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Detected file endianness.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert_eq!(reader.endian(), mrc::FileEndian::LittleEndian);
    /// # Ok(())
    /// # }
    /// ```
    pub fn endian(&self) -> FileEndian {
        self.endian
    }

    /// Raw voxel data bytes.
    ///
    /// For memory-mapped readers this returns a zero-copy `&[u8]` view.
    /// For buffered readers this borrows from the internal `Vec<u8>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![42u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let bytes = reader.raw_bytes();
    /// assert_eq!(bytes.len(), 64);
    /// # Ok(())
    /// # }
    /// ```
    pub fn raw_bytes(&self) -> &[u8] {
        match &self.source {
            DataSource::Buffered { data, .. } => data,
            #[cfg(feature = "mmap")]
            DataSource::Mmap {
                map, data_offset, ..
            } => {
                let data_size = self.header.data_size().unwrap_or(0);
                let end = data_offset + data_size;
                if end > map.len() {
                    &map[*data_offset..]
                } else {
                    &map[*data_offset..end]
                }
            }
        }
    }

    /// Extended header bytes (empty slice if none).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert!(reader.ext_header_bytes().is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn ext_header_bytes(&self) -> &[u8] {
        if !self.ext_header.is_empty() {
            return &self.ext_header;
        }
        // For mmap readers, ext_header is empty because ext bytes are in the map.
        // We parse them from the map on demand.
        let ext_size = self.header.nsymbt.max(0) as usize;
        if ext_size == 0 {
            return &[];
        }
        match &self.source {
            #[cfg(feature = "mmap")]
            DataSource::Mmap { map, .. } => {
                if 1024 + ext_size <= map.len() {
                    &map[1024..1024 + ext_size]
                } else {
                    &[]
                }
            }
            _ => &[],
        }
    }

    /// Returns `true` when the file is shorter than the header's declared data
    /// size (only possible when opened in permissive mode).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert!(!reader.is_truncated());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_truncated(&self) -> bool {
        match &self.source {
            DataSource::Buffered { truncated, .. } => *truncated,
            #[cfg(feature = "mmap")]
            DataSource::Mmap { truncated, .. } => *truncated,
        }
    }

    // ── Volume type queries (delegated to header) ────────────────────

    /// Returns `true` if the file represents a single 2D image (`nz == 1`).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert!(reader.is_single_image());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_single_image(&self) -> bool {
        self.header.is_single_image()
    }

    /// Returns `true` if the file is an image stack (`ispg == 0`).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # h.ispg = 0;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 256]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert!(reader.is_image_stack());
    /// assert!(!reader.is_volume());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_image_stack(&self) -> bool {
        self.header.is_image_stack()
    }

    /// Returns `true` if the file represents a single 3D volume.
    ///
    /// This is `true` when the file is neither an image stack nor a volume stack.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 4;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 256]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert!(reader.is_volume());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_volume(&self) -> bool {
        self.header.is_volume()
    }

    /// Returns `true` if the file is a volume stack (`ispg` in 400..=630).
    ///
    /// Volume stacks store multiple sub-volumes contiguously in Z.    ///
    /// Use [`volumes`](Self::volumes) to iterate over sub-volumes.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 8;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # h.ispg = 401;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 512]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert!(reader.is_volume_stack());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_volume_stack(&self) -> bool {
        self.header.is_volume_stack()
    }

    /// Return the logical 4D shape `[nvolumes, mz, ny, nx]`.
    ///
    /// For non-stack files this is `[1, nz, ny, nx]`.
    /// For volume stacks it is `[nz / mz, mz, ny, nx]`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 64; h.ny = 64; h.nz = 60;
    /// # h.mx = 64; h.my = 64; h.mz = 30;
    /// # h.ispg = 401;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64 * 64 * 60 * 4]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// assert_eq!(reader.logical_shape(), [2, 30, 64, 64]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn logical_shape(&self) -> [usize; 4] {
        self.header.logical_shape()
    }

    /// Cross-check header statistics against actual data (1% tolerance).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![0u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// reader.validate_header_stats()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        crate::engine::stats::validate_header_stats(&self.header, self.raw_bytes())
    }
}

// ============================================================================
// Read block methods
// ============================================================================

impl Reader {
    /// Read a block of raw voxel bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let data = vec![0u8; 64];
    /// # let buf: Vec<u8> = raw.into_iter().chain(data).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let block = reader.read_block_bytes([0, 0, 0], [4, 4, 1])?;
    /// assert_eq!(block.len(), 64);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        let data = self._source_data();
        let data_len = data.len();
        crate::io::reader_common::validate_block_bounds(
            self.shape,
            self.mode(),
            data_len,
            offset,
            shape,
        )?;
        Ok(crate::io::reader_common::gather_block_bytes(
            data,
            self.shape,
            self.mode(),
            offset,
            shape,
        ))
    }

    /// Zero-copy read of a contiguous Z-slab as `&[T]`.
    ///
    /// Zero-copy access is supported when both of these hold:
    /// - The file's endianness matches the host (native endian).
    /// - The requested type `T` matches the file's on-disk mode.
    ///
    /// When available, this method returns `&[T]` directly into the internal
    /// data buffer — no allocation, no memcpy. This works for **both**
    /// memory-mapped and buffered readers (including compressed files).
    ///
    /// When the conditions are *not* met, returns [`Error::TypeMismatch`] or
    /// [`Error::ModeMismatch`]. Use [`subregion`](Self::subregion) as a
    /// fallback that handles all modes and endianness via copying.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let reader = mrc::Reader::open("density.mrc")?;
    /// let slab: &[f32] = reader.slab_as::<f32>(0, 1)?;
    /// println!("Slab has {} voxels", slab.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// With a buffered reader (e.g. from a byte buffer):
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let bytes: Vec<u8> = std::fs::read("density.mrc")?;
    /// let reader = mrc::Reader::from_bytes(bytes)?;
    /// let slab: &[f32] = reader.slab_as::<f32>(0, 1)?;
    /// println!("Slab has {} voxels", slab.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn slab_as<T: Voxel>(&self, z: usize, k: usize) -> Result<&[T], Error> {
        #[cfg(feature = "mmap")]
        if let DataSource::Mmap {
            map, data_offset, ..
        } = &self.source
        {
            return self._slab_as_mmap::<T>(map, *data_offset, z, k);
        }

        // Buffered fast path: zero-copy reinterpret when native endian + matching type.
        if let DataSource::Buffered { data, .. } = &self.source {
            if T::MODE != self.mode() {
                return Err(Error::ModeMismatch {
                    file_mode: self.mode(),
                    requested_mode: T::MODE,
                    offset: None,
                });
            }
            if !self.endian.is_native() {
                return Err(Error::TypeMismatch {
                    expected: T::BYTE_SIZE,
                    actual: 0,
                });
            }

            let b = T::BYTE_SIZE;
            let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];

            if k == 0 || z + k > nz {
                return Err(Error::bounds_err());
            }

            let linear_start = z * nx * ny;
            let byte_start = linear_start * b;
            let count = nx * ny * k;
            let byte_end = byte_start + count * b;

            if byte_end > data.len() {
                return Err(Error::bounds_err());
            }

            let ptr = data.as_ptr();
            let abs_offset = ptr as usize + byte_start;
            if abs_offset % core::mem::align_of::<T>() != 0 {
                return Err(Error::TypeMismatch {
                    expected: core::mem::align_of::<T>(),
                    actual: abs_offset % core::mem::align_of::<T>(),
                });
            }

            unsafe {
                // SAFETY: byte_start ≤ data.len() and count*b ≤ data.len()-byte_start
                // (verified by byte_end check above). ptr.add(byte_start) is therefore
                // in-bounds and produces count*BYTE_SIZE valid bytes. Alignment verified
                // by the abs_offset % align_of::<T>() check above.
                let p = ptr.add(byte_start) as *const T;
                return Ok(core::slice::from_raw_parts(p, count));
            }
        }

        Err(Error::TypeMismatch {
            expected: 0,
            actual: 0,
        })
    }

    #[cfg(feature = "mmap")]
    fn _slab_as_mmap<'a, T: Voxel>(
        &self,
        map: &'a memmap2::Mmap,
        data_offset: usize,
        z: usize,
        k: usize,
    ) -> Result<&'a [T], Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
                offset: None,
            });
        }
        if !self.endian.is_native() {
            return Err(Error::TypeMismatch {
                expected: T::BYTE_SIZE,
                actual: 0,
            });
        }

        let b = T::BYTE_SIZE;
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];

        if k == 0 || z + k > nz {
            return Err(Error::bounds_err());
        }

        let linear_start = z * nx * ny;
        let byte_start = data_offset + linear_start * b;
        let count = nx * ny * k;
        let byte_end = byte_start + count * b;

        if byte_end > map.len() {
            return Err(Error::bounds_err());
        }

        if byte_start % core::mem::align_of::<T>() != 0 {
            return Err(Error::TypeMismatch {
                expected: core::mem::align_of::<T>(),
                actual: byte_start % core::mem::align_of::<T>(),
            });
        }

        unsafe {
            // SAFETY: byte_start ≤ map.len() and count*b ≤ map.len()-byte_start
            // (verified by byte_end check above). Mmap is page-aligned (≥ 4 KiB),
            // so byte_start % align_of::<T>() suffices (verified above). The returned
            // &[T] borrows from 'a via the mmap reference.
            let ptr = map.as_ptr().add(byte_start) as *const T;
            Ok(core::slice::from_raw_parts(ptr, count))
        }
    }

    pub(crate) fn read_block_bytes_cow<'a>(
        &'a self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Cow<'a, [u8]>, Error> {
        match &self.source {
            DataSource::Buffered { data, .. } => {
                let data_len = data.len();
                crate::io::reader_common::validate_block_bounds(
                    self.shape,
                    self.mode(),
                    data_len,
                    offset,
                    shape,
                )?;

                let [nx, ny, ..] = [self.shape.nx, self.shape.ny, self.shape.nz];
                let [ox, oy, oz] = offset;
                let [sx, sy, sz] = shape;

                if ox == 0 && sx == nx && oy == 0 && sy == ny {
                    let (start, byte_len) = if self.mode == Mode::Packed4Bit {
                        let row_bytes = nx.div_ceil(2);
                        (oz * ny * row_bytes, row_bytes * ny * sz)
                    } else {
                        let linear = oz * nx * ny;
                        let b = self.mode.byte_size();
                        (linear * b, sx * sy * sz * b)
                    };
                    return Ok(Cow::Borrowed(&data[start..start + byte_len]));
                }

                Ok(Cow::Owned(crate::io::reader_common::gather_block_bytes(
                    data,
                    self.shape,
                    self.mode(),
                    offset,
                    shape,
                )))
            }
            #[cfg(feature = "mmap")]
            DataSource::Mmap {
                map, data_offset, ..
            } => {
                let [nx, ny, ..] = [self.shape.nx, self.shape.ny, self.shape.nz];
                let [ox, oy, oz] = offset;
                let [sx, sy, sz] = shape;
                let data_len = map.len().saturating_sub(*data_offset);
                crate::io::reader_common::validate_block_bounds(
                    self.shape,
                    self.mode(),
                    data_len,
                    offset,
                    shape,
                )?;

                if ox == 0 && sx == nx && oy == 0 && sy == ny {
                    let (start_offset, byte_len) = if self.mode == Mode::Packed4Bit {
                        let row_bytes = nx.div_ceil(2);
                        (data_offset + oz * ny * row_bytes, row_bytes * ny * sz)
                    } else {
                        let linear = oz * nx * ny;
                        let b = self.mode.byte_size();
                        (data_offset + linear * b, sx * sy * sz * b)
                    };
                    return Ok(Cow::Borrowed(&map[start_offset..start_offset + byte_len]));
                }

                Ok(Cow::Owned(crate::io::reader_common::gather_block_bytes(
                    &map[*data_offset..],
                    self.shape,
                    self.mode(),
                    offset,
                    shape,
                )))
            }
        }
    }

    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        crate::io::reader_common::decode_block(bytes, self.mode(), self.endian)
    }

    /// Internal: return a `&[u8]` to the full data region regardless of backend.
    fn _source_data(&self) -> &[u8] {
        match &self.source {
            DataSource::Buffered { data, .. } => data,
            #[cfg(feature = "mmap")]
            DataSource::Mmap {
                map, data_offset, ..
            } => &map[*data_offset..],
        }
    }

    // ── Iteration methods ─────────────────────────────────────────────

    /// Iterate over Z-slices.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 2;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 128]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for slice in reader.slices::<f32>() {
    ///     let block = slice?;
    ///     println!("Slice at z={} has {} voxels", block.offset[2], block.data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn slices<T: Voxel>(&self) -> crate::iter::RegionIter<'_, T, crate::iter::SliceStepper> {
        crate::iter::RegionIter::with_stepper(
            self,
            self.shape(),
            crate::iter::SliceStepper::default(),
        )
    }

    /// Iterate over Z-slabs.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 4;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 256]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for slab in reader.slabs::<f32>(2) {
    ///     let block = slab?;
    ///     println!("Slab at z={} depth={}", block.offset[2], block.shape[2]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn slabs<T: Voxel>(
        &self,
        k: usize,
    ) -> crate::iter::RegionIter<'_, T, crate::iter::SlabStepper> {
        crate::iter::RegionIter::with_stepper(self, self.shape(), crate::iter::SlabStepper::new(k))
    }

    /// Iterate over 3D tiles.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 4;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 256]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for tile in reader.tiles::<f32>([2, 2, 2])? {
    ///     let block = tile?;
    ///     println!("Tile: {:?}", block.shape);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn tiles<T: Voxel>(
        &self,
        tile_shape: [usize; 3],
    ) -> Result<crate::iter::RegionIter<'_, T, crate::iter::TileStepper>, Error> {
        Ok(crate::iter::RegionIter::with_stepper(
            self,
            self.shape(),
            crate::iter::TileStepper::new(tile_shape)?,
        ))
    }

    /// Iterate over sub-volumes (volume stacks only).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # h.ispg = 401;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 256]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for vol in reader.volumes::<f32>()? {
    ///     let block = vol?;
    ///     println!("Volume depth: {}", block.shape[2]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn volumes<T: Voxel>(
        &self,
    ) -> Result<crate::iter::RegionIter<'_, T, crate::iter::SlabStepper>, Error> {
        let mz = self.header().mz.max(0) as usize;
        if !self.header().is_volume_stack() || mz == 0 {
            return Err(Error::NotAVolumeStack {
                ispg: self.header().ispg,
                mz: self.header().mz,
            });
        }
        Ok(self.slabs(mz))
    }

    /// Read a single sub-region.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 2;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 128]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let block = reader.subregion::<f32>([0, 0, 0], [2, 2, 1])?;
    /// assert_eq!(block.data.len(), 4);
    /// # Ok(())
    /// # }
    /// ```
    pub fn subregion<T: Voxel>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes_cow(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(VoxelBlock {
            offset,
            shape,
            data,
        })
    }

    /// Read the entire volume.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 2;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 128]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let volume = reader.read_volume::<f32>()?;
    /// assert_eq!(volume.data.len(), 32);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_volume<T: Voxel>(&self) -> Result<VoxelBlock<T>, Error> {
        self.subregion([0, 0, 0], [self.shape.nx, self.shape.ny, self.shape.nz])
    }

    /// Iterate over Z-slices as u8 (Uint16 narrowing or Packed4Bit unpack).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 2;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # h.mode = 6; // Uint16
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for slice in reader.slices_u8() {
    ///     let block = slice?;
    ///     println!("u8 slice at z={}", block.offset[2]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn slices_u8(&self) -> crate::io::reader_common::VoxelIter<'_, u8> {
        if self.mode() == Mode::Packed4Bit {
            let shape = self.shape();
            let nx = shape.nx;
            let ny = shape.ny;
            let nz = shape.nz;
            return Box::new((0..nz).map(move |z| {
                let bytes = self.read_block_bytes_cow([0, 0, z], [nx, ny, 1])?;
                let data = crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, nx, ny);
                Ok(VoxelBlock {
                    offset: [0, 0, z],
                    shape: [nx, ny, 1],
                    data,
                })
            }));
        }
        if self.mode() != Mode::Uint16 {
            return Box::new(std::iter::once(Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
                offset: None,
            })));
        }
        Box::new(self.slices::<u16>().map(|b| {
            let b = b?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_u16_slice_to_u8(&b.data)?,
            })
        }))
    }

    /// Iterate over Z-slabs as u8.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 4;
    /// # h.mode = 6; // Uint16
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 128]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for slab in reader.slabs_u8(2) {
    ///     let block = slab?;
    ///     println!("u8 slab depth: {}", block.shape[2]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn slabs_u8(&self, k: usize) -> crate::io::reader_common::VoxelIter<'_, u8> {
        if self.mode() == Mode::Packed4Bit {
            let volume_shape = self.shape();
            let nx = volume_shape.nx;
            let ny = volume_shape.ny;
            let k = k.max(1);
            let mut z = 0usize;
            return Box::new(std::iter::from_fn(move || {
                if z >= volume_shape.nz {
                    return None;
                }
                let start = z;
                let sz = k.min(volume_shape.nz - z);
                z += sz;
                let bytes = match self.read_block_bytes_cow([0, 0, start], [nx, ny, sz]) {
                    Ok(b) => b,
                    Err(e) => return Some(Err(e)),
                };
                let data = crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, nx, ny * sz);
                Some(Ok(VoxelBlock {
                    offset: [0, 0, start],
                    shape: [nx, ny, sz],
                    data,
                }))
            }));
        }
        if self.mode() != Mode::Uint16 {
            return Box::new(std::iter::once(Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
                offset: None,
            })));
        }
        let k = k.max(1);
        Box::new(self.slabs::<u16>(k).map(|b| {
            let b = b?;
            Ok(VoxelBlock {
                offset: b.offset,
                shape: b.shape,
                data: crate::engine::convert::convert_u16_slice_to_u8(&b.data)?,
            })
        }))
    }

    /// Iterate over Z-slices of a Mode 0 file with configurable signed/unsigned interpretation.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 2;
    /// # h.mx = 4; h.my = 4; h.mz = 2;
    /// # h.mode = 0; // Int8
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 32]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for slice in reader.slices_mode0(mrc::M0Interpretation::Signed) {
    ///     let block = slice?;
    ///     println!("Mode 0 slice at z={}", block.offset[2]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn slices_mode0(
        &self,
        interp: crate::M0Interpretation,
    ) -> crate::io::reader_common::VoxelIter<'_, f32> {
        if self.mode() != Mode::Int8 {
            return Box::new(std::iter::once(Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Int8,
                offset: None,
            })));
        }
        let volume_shape = self.shape();
        Box::new((0..volume_shape.nz).map(move |z| {
            let bytes =
                self.read_block_bytes_cow([0, 0, z], [volume_shape.nx, volume_shape.ny, 1])?;
            let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
            Ok(VoxelBlock {
                offset: [0, 0, z],
                shape: [volume_shape.nx, volume_shape.ny, 1],
                data,
            })
        }))
    }

    /// Iterate over Z-slabs of a Mode 0 file.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 4;
    /// # h.mx = 4; h.my = 4; h.mz = 4;
    /// # h.mode = 0; // Int8
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// for slab in reader.slabs_mode0(2, mrc::M0Interpretation::Signed) {
    ///     let block = slab?;
    ///     println!("Mode 0 slab depth: {}", block.shape[2]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn slabs_mode0(
        &self,
        k: usize,
        interp: crate::M0Interpretation,
    ) -> crate::io::reader_common::VoxelIter<'_, f32> {
        if self.mode() != Mode::Int8 {
            return Box::new(std::iter::once(Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Int8,
                offset: None,
            })));
        }
        let volume_shape = self.shape();
        let k = k.max(1);
        let mut z = 0usize;
        Box::new(std::iter::from_fn(move || {
            if z >= volume_shape.nz {
                return None;
            }
            let start = z;
            let sz = k.min(volume_shape.nz - z);
            z += sz;
            let bytes = match self
                .read_block_bytes_cow([0, 0, start], [volume_shape.nx, volume_shape.ny, sz])
            {
                Ok(b) => b,
                Err(e) => return Some(Err(e)),
            };
            let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
            Some(Ok(VoxelBlock {
                offset: [0, 0, start],
                shape: [volume_shape.nx, volume_shape.ny, sz],
                data,
            }))
        }))
    }

    /// Return a wrapper that auto-converts all reads to type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let converter = reader.convert::<f32>();
    /// for slice in converter.slices() {
    ///     let block = slice?;
    ///     println!("Converted slice: {} voxels", block.data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn convert<T>(&self) -> crate::io::reader_common::ConvertReader<'_, T>
    where
        T: Voxel + crate::engine::convert::ConvertFrom<f32>,
    {
        let m0_interp = if self.mode() == Mode::Int8 {
            if let Some(imod) = self.header().detect_imod() {
                if !imod.bytes_are_signed {
                    crate::M0Interpretation::Unsigned
                } else {
                    crate::M0Interpretation::Signed
                }
            } else {
                crate::M0Interpretation::Signed
            }
        } else {
            crate::M0Interpretation::Signed
        };

        crate::io::reader_common::ConvertReader {
            reader: self,
            complex_strategy: crate::ComplexToRealStrategy::Magnitude,
            m0_interp,
            _target: std::marker::PhantomData,
        }
    }

    /// Read the entire volume as an ndarray (requires `ndarray` feature).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), mrc::Error> {
    /// let reader = mrc::Reader::open("density.mrc")?;
    /// let array = reader.to_ndarray::<f32>()?;
    /// println!("Array shape: {:?}", array.shape());
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "ndarray")]
    pub fn to_ndarray<T: Voxel>(&self) -> Result<ndarray::Array3<T>, Error> {
        let block = self.read_volume::<T>()?;
        ndarray::Array3::from_shape_vec([self.shape.nz, self.shape.ny, self.shape.nx], block.data)
            .map_err(|_| Error::bounds_err())
    }

    /// Read the entire volume as u8 (Packed4Bit unpack).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # h.mode = 101; // Packed4Bit
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # // Packed4Bit data: ceil(4/2)*4*1 = 8 bytes
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 8]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let block = reader.read_volume_u8()?;
    /// assert_eq!(block.data.len(), 16);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_volume_u8(&self) -> Result<VoxelBlock<u8>, Error> {
        if self.mode() != Mode::Packed4Bit {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Packed4Bit,
                offset: None,
            });
        }
        let shape = self.shape();
        let block_shape = [shape.nx, shape.ny, shape.nz];
        let bytes = self.read_block_bytes_cow([0, 0, 0], block_shape)?;
        let data =
            crate::engine::convert::unpack_u4_bytes_to_u8(&bytes, shape.nx, shape.ny * shape.nz);
        Ok(VoxelBlock {
            offset: [0, 0, 0],
            shape: block_shape,
            data,
        })
    }

    // ── Extended header convenience methods ──────────────────────────

    /// Auto-dispatch extended header parsing.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let ext = reader.parse_extended_header();
    /// assert_eq!(ext, mrc::ExtHeaderData::None);
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse_extended_header(&self) -> crate::ExtHeaderData {
        crate::ExtHeaderData::from_header(&self.header, self.ext_header_bytes())
    }

    /// Parse FEI1 metadata records.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let fei1 = reader.fei1_metadata();
    /// assert!(fei1.is_none()); // no FEI1 extended header present
    /// # Ok(())
    /// # }
    /// ```
    pub fn fei1_metadata(&self) -> Option<Vec<crate::Fei1Metadata>> {
        if crate::ExtHeaderType::from_header(&self.header) != crate::ExtHeaderType::Fei1 {
            return None;
        }
        crate::parse_fei1_records(self.ext_header_bytes())
    }

    /// Parse FEI2 metadata records.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let fei2 = reader.fei2_metadata();
    /// assert!(fei2.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn fei2_metadata(&self) -> Option<Vec<crate::Fei2Metadata>> {
        if crate::ExtHeaderType::from_header(&self.header) != crate::ExtHeaderType::Fei2 {
            return None;
        }
        crate::parse_fei2_records(self.ext_header_bytes())
    }

    /// Parse CCP4 symmetry records.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let ccp4 = reader.ccp4_records();
    /// assert!(ccp4.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn ccp4_records(&self) -> Option<Vec<crate::Ccp4Record>> {
        if crate::ExtHeaderType::from_header(&self.header) != crate::ExtHeaderType::Ccp4 {
            return None;
        }
        crate::parse_ccp4_records(self.ext_header_bytes())
    }

    /// Parse MRCO legacy records.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let mrco = reader.mrco_records();
    /// assert!(mrco.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn mrco_records(&self) -> Option<Vec<crate::MrcoRecord>> {
        if crate::ExtHeaderType::from_header(&self.header) != crate::ExtHeaderType::Mrco {
            return None;
        }
        crate::parse_mrco_records(self.ext_header_bytes())
    }

    /// Parse SerialEM records.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let seri = reader.seri_records();
    /// assert!(seri.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn seri_records(&self) -> Option<Vec<crate::SeriRecord>> {
        if crate::ExtHeaderType::from_header(&self.header) != crate::ExtHeaderType::Seri {
            return None;
        }
        crate::parse_seri_records(self.ext_header_bytes())
    }

    /// Parse Agard records.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let agar = reader.agar_records();
    /// assert!(agar.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn agar_records(&self) -> Option<Vec<crate::AgarRecord>> {
        if crate::ExtHeaderType::from_header(&self.header) != crate::ExtHeaderType::Agar {
            return None;
        }
        crate::parse_agar_records(self.ext_header_bytes())
    }

    /// Parse IMOD metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), mrc::Error> {
    /// # let mut h = mrc::Header::new();
    /// # h.nx = 4; h.ny = 4; h.nz = 1;
    /// # h.mx = 4; h.my = 4; h.mz = 1;
    /// # let mut raw = [0u8; 1024];
    /// # h.encode_to_bytes(&mut raw);
    /// # let buf: Vec<u8> = raw.into_iter().chain(vec![0u8; 64]).collect();
    /// # let reader = mrc::Reader::from_bytes(buf)?;
    /// let imod = reader.imod_metadata();
    /// assert!(imod.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn imod_metadata(&self) -> Option<crate::ImodMetadata> {
        crate::parse_imod_metadata(&self.header)
    }
}
