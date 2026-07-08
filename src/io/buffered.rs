//! In-memory buffered MRC file reader.
//!
//! Provides [`Reader`], which loads the entire file into a `Vec<u8>` on open.
//! This enables fast random access to any slice or block, but requires enough
//! RAM to hold the full dataset.
//!
//! For large files that do not fit in RAM, consider [`MmapReader`](crate::MmapReader)
//! (requires the `mmap` feature).

use crate::engine::block::VolumeShape;
use crate::engine::endian::FileEndian;
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

/// In-memory buffered MRC file reader.
///
/// The entire file is read into memory on open, making this suitable for
/// smaller files or when random access to any slice is needed.
///
/// For large files that don't fit in RAM, consider [`MmapReader`](crate::MmapReader)
/// (requires the `mmap` feature).
///
/// # Example
///
/// ```no_run
/// use mrc::Reader;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let reader = Reader::open("protein.mrc")?;
///     for slice in reader.slices::<f32>() {
///         let block = slice?;
///         // block.data is Vec<f32>
///     }
///     Ok(())
/// }
/// ```
///
/// ## Opening compressed files
///
/// [`Reader::open`] auto-detects gzip and bzip2 compression from magic bytes.
/// Use [`open_plain`](Self::open_plain) to force plain (uncompressed) reading,
/// [`open_gzip`](Self::open_gzip) for gzip-only, or
/// [`open_bzip2`](Self::open_bzip2) for bzip2-only.
#[derive(Debug)]
pub struct Reader {
    pub(crate) header: Header,
    pub(crate) ext_header: Vec<u8>,
    pub(crate) data: Vec<u8>,
    pub(crate) endian: FileEndian,
    pub(crate) mode: Mode,
    pub(crate) shape: VolumeShape,
}

impl Reader {
    /// Open an MRC file, auto-detecting gzip/bzip2 compression from magic bytes.
    ///
    /// For plain files this is equivalent to [`open_plain`](Self::open_plain).
    /// For gzip files it delegates to [`open_gzip`](Self::open_gzip); for bzip2
    /// files to [`open_bzip2`](Self::open_bzip2).
    ///
    /// **Decompression safety:** Gzip and bzip2 files are decompressed with a
    /// hard cap of [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB) to prevent
    /// decompression bombs. Use [`open_gzip_with_limit`](Self::open_gzip_with_limit)
    /// or [`open_bzip2_with_limit`](Self::open_bzip2_with_limit) for a custom limit.
    ///
    /// **Note:** gzip and bzip2 support require the `gzip` and `bzip2` feature
    /// flags respectively. Both are enabled by default. If a compressed file is
    /// opened without the corresponding feature, it will be misinterpreted as
    /// plain and fail with [`InvalidHeader`](crate::Error::InvalidHeader).
    ///
    /// [`DEFAULT_MAX_DECOMPRESSED_BYTES`]: crate::DEFAULT_MAX_DECOMPRESSED_BYTES
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        use std::io::{Read, Seek};

        // Open the file once and peek at the magic bytes to avoid a redundant
        // File::open in the plain / compressed constructors below.
        let mut file = std::fs::File::open(&path)?;
        let mut magic = [0u8; 2];
        let n = file.read(&mut magic)?;

        if n >= 2 {
            match magic {
                #[cfg(feature = "gzip")]
                [0x1f, 0x8b] => {
                    let _ = file.seek(std::io::SeekFrom::Start(0));
                    return Self::_open_gzip_file(
                        file,
                        false,
                        crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
                    )
                    .map(|(r, _)| r);
                }
                #[cfg(feature = "bzip2")]
                [b'B', b'Z'] => {
                    let _ = file.seek(std::io::SeekFrom::Start(0));
                    return Self::_open_bzip2_file(
                        file,
                        false,
                        crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
                    )
                    .map(|(r, _)| r);
                }
                _ => {}
            }
        }

        // Plain file — seek back to the start and parse.
        let _ = file.seek(std::io::SeekFrom::Start(0));
        Self::_open_plain_file(file, false).map(|(r, _)| r)
    }

    /// Open a plain (uncompressed) MRC file.
    pub fn open_plain<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Self::_open_plain(path, false).map(|(r, _)| r)
    }

    /// Open in **permissive** mode, auto-detecting compression.
    ///
    /// Non-fatal header issues (unusual MAP field, unexpected `nversion`,
    /// non-standard axis mapping, etc.) are collected as warning strings
    /// instead of causing a hard error. Only genuinely unreadable files
    /// (negative dimensions, unsupported mode, IO failure) return `Err`.
    pub fn open_permissive<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<(Self, Vec<String>), Error> {
        use std::io::{Read, Seek};

        let mut file = std::fs::File::open(&path)?;
        let mut magic = [0u8; 2];
        let n = file.read(&mut magic)?;

        if n >= 2 {
            match magic {
                #[cfg(feature = "gzip")]
                [0x1f, 0x8b] => {
                    let _ = file.seek(std::io::SeekFrom::Start(0));
                    return Self::_open_gzip_file(
                        file,
                        true,
                        crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
                    );
                }
                #[cfg(feature = "bzip2")]
                [b'B', b'Z'] => {
                    let _ = file.seek(std::io::SeekFrom::Start(0));
                    return Self::_open_bzip2_file(
                        file,
                        true,
                        crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
                    );
                }
                _ => {}
            }
        }

        let _ = file.seek(std::io::SeekFrom::Start(0));
        Self::_open_plain_file(file, true)
    }

    /// Parse a plain (uncompressed) MRC file from an already-opened file handle.
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

        Self::_build_reader(header, ext_header, data, warnings)
    }

    fn _open_plain<P: AsRef<std::path::Path>>(
        path: P,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_plain_file(std::fs::File::open(path)?, permissive)
    }

    /// Read an MRC file from any [`Read`] source (in-memory buffer, network stream, etc.).
    ///
    /// The entire source is read into memory, then parsed. For file-backed reading,
    /// prefer [`open`](Self::open) which can use memory-mapped I/O for large files.
    ///
    /// # Example
    /// ```
    /// use mrc::{Header, Reader};
    /// use std::io::Cursor;
    ///
    /// // Create a minimal valid MRC file in memory
    /// let mut header = Header::new();
    /// header.nx = 4; header.ny = 4; header.nz = 1;
    /// header.mx = 4; header.my = 4; header.mz = 1;
    /// header.mode = 2;
    /// let mut header_bytes = [0u8; 1024];
    /// header.encode_to_bytes(&mut header_bytes);
    /// let mut bytes = header_bytes.to_vec();
    /// bytes.extend_from_slice(&[0u8; 64]); // 4×4×1 Float32 = 64 bytes
    ///
    /// let reader = Reader::from_reader(Cursor::new(bytes)).unwrap();
    /// assert_eq!(reader.shape().nx, 4);
    /// ```
    pub fn from_reader<R: std::io::Read>(mut reader: R) -> Result<Self, Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Self::_read_from_buf(buf, false).map(|(r, _)| r)
    }

    /// Read from any [`Read`] source in permissive mode.
    pub fn from_reader_permissive<R: std::io::Read>(
        mut reader: R,
    ) -> Result<(Self, Vec<String>), Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Self::_read_from_buf(buf, true)
    }

    /// Parse an MRC file directly from an in-memory byte buffer.
    ///
    /// This is useful when the data is already in memory (e.g. from a camera
    /// readout, embedded resource, or downloaded blob) and avoids an extra
    /// copy through [`Cursor`](std::io::Cursor).
    ///
    /// # Example
    /// ```
    /// use mrc::{Header, Reader};
    ///
    /// // Create a minimal valid MRC file in memory
    /// let mut header = Header::new();
    /// header.nx = 4; header.ny = 4; header.nz = 1;
    /// header.mx = 4; header.my = 4; header.mz = 1;
    /// header.mode = 2;
    /// let mut header_bytes = [0u8; 1024];
    /// header.encode_to_bytes(&mut header_bytes);
    /// let mut bytes = header_bytes.to_vec();
    /// bytes.extend_from_slice(&[0u8; 64]); // 4×4×1 Float32 = 64 bytes
    ///
    /// let reader = Reader::from_bytes(bytes).unwrap();
    /// assert_eq!(reader.shape().nx, 4);
    /// ```
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, Error> {
        Self::_read_from_buf(data, false).map(|(r, _)| r)
    }

    /// Parse an MRC file from an in-memory byte buffer in permissive mode.
    pub fn from_bytes_permissive(data: Vec<u8>) -> Result<(Self, Vec<String>), Error> {
        Self::_read_from_buf(data, true)
    }

    /// Parse an MRC file from an in-memory byte buffer (internal, with permissive flag).
    fn _read_from_buf(data: Vec<u8>, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        if data.len() < 1024 {
            return Err(Error::InvalidHeader);
        }
        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&data[..1024]);
        let (header, warnings, _endian, data_size) =
            crate::io::reader_common::parse_header(&header_bytes, permissive)?;

        let ext_size = header.nsymbt as usize;
        let ext_header = if ext_size > 0 && 1024 + ext_size <= data.len() {
            data[1024..1024 + ext_size].to_vec()
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

        Self::_build_reader(header, ext_header, voxel_data, warnings)
    }

    /// Construct a Reader from parsed header + data, detecting IMOD unsigned Mode 0.
    fn _build_reader(
        header: Header,
        ext_header: Vec<u8>,
        data: Vec<u8>,
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
                data,
                endian,
                mode,
                shape,
            },
            warnings,
        ))
    }

    /// Volume dimensions of the opened file.
    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    /// Voxel data mode of the opened file.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// A reference to the parsed MRC header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Read a block of raw voxel bytes from the file.
    ///
    /// Supports arbitrary sub-blocks; non-contiguous regions are gathered
    /// into a contiguous buffer automatically.
    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        crate::io::reader_common::validate_block_bounds(
            self.shape,
            self.mode(),
            self.data.len(),
            offset,
            shape,
        )?;
        Ok(crate::io::reader_common::gather_block_bytes(
            &self.data,
            self.shape,
            self.mode(),
            offset,
            shape,
        ))
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// # Errors
    /// Returns `Error::ModeMismatch` if `T` does not match the file mode.
    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        crate::io::reader_common::decode_block(bytes, self.mode(), self.endian)
    }
}

impl Reader {
    /// Get a reference to the raw data bytes
    pub fn data_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Cross-check header statistics against actual data.
    ///
    /// Computes `dmin`, `dmax`, `dmean` and `rms` from the data block and
    /// compares them with the header values using a 1% relative tolerance
    /// (matching Python `mrcfile`'s `np.isclose(rtol=0.01)`).
    ///
    /// # Errors
    /// Returns [`Error::StatsMismatch`] if any statistic deviates by more than 1 %.
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        crate::engine::stats::validate_header_stats(&self.header, &self.data)
    }

    /// Get the file endianness
    pub fn endian(&self) -> FileEndian {
        self.endian
    }

    /// Get the extended header bytes, if any.
    pub fn ext_header_bytes(&self) -> &[u8] {
        &self.ext_header
    }
}
