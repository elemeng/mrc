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

use std::vec::Vec;

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
        match crate::io::reader::detect_compression(&path)? {
            crate::io::reader::CompressionType::Plain => Self::open_plain(path),
            #[cfg(feature = "gzip")]
            crate::io::reader::CompressionType::Gzip => Self::open_gzip(path),
            #[cfg(feature = "bzip2")]
            crate::io::reader::CompressionType::Bzip2 => Self::open_bzip2(path),
        }
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
        match crate::io::reader::detect_compression(&path)? {
            crate::io::reader::CompressionType::Plain => Self::_open_plain(path, true),
            #[cfg(feature = "gzip")]
            crate::io::reader::CompressionType::Gzip => Self::open_gzip_permissive(path),
            #[cfg(feature = "bzip2")]
            crate::io::reader::CompressionType::Bzip2 => Self::open_bzip2_permissive(path),
        }
    }

    fn _open_plain<P: AsRef<std::path::Path>>(
        path: P,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;

        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

        let (header, warnings, endian, data_size) =
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

        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);
        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;

        // Detect IMOD unsigned Mode 0
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

    /// Read and decode a block of voxels to the specified type.
    ///
    /// Returns an error if `T` does not match the file's voxel mode.
    ///
    /// Use [`subregion`](crate::ReaderMethods::subregion) instead — it is
    /// available on all reader types and behaves identically.
    #[deprecated(since = "0.2.4", note = "use `subregion` instead")]
    pub fn read_block<T: Voxel>(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<crate::engine::block::VoxelBlock<T>, Error> {
        self.subregion(offset, shape)
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
    /// compares them with the header values using a 1 % relative tolerance
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
