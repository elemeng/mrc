//! Bzip2-compressed MRC file reader and writer.
//!
//! Because bzip2 does not support random access, the writer buffers the entire
//! file in memory and compresses on [`CompressedWriter::finalize`]. This matches
//! the behaviour of the reference Python `mrcfile` library.

use crate::engine::block::VolumeShape;
use crate::{Error, Header};

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::vec::Vec;

/// Bzip2-compressed MRC file reader.
///
/// This is a thin newtype wrapper around [`Reader`](crate::Reader). All
/// [`Reader`](crate::Reader) methods are available via [`Deref`](std::ops::Deref).
#[derive(Debug)]
pub struct Bzip2Reader(pub crate::Reader);

impl std::ops::Deref for Bzip2Reader {
    type Target = crate::Reader;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Bzip2Reader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Bzip2Reader {
    /// Open a bzip2-compressed MRC file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        crate::Reader::open_bzip2(path).map(Self)
    }

    /// Open a bzip2-compressed MRC file in **permissive** mode.
    pub fn open_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        crate::Reader::open_bzip2_permissive(path).map(|(r, w)| (Self(r), w))
    }
}

impl crate::Reader {
    /// Open a bzip2-compressed MRC file.
    pub fn open_bzip2<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::_open_bzip2(path, false).map(|(r, _)| r)
    }

    /// Open a bzip2-compressed MRC file in **permissive** mode.
    pub fn open_bzip2_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        Self::_open_bzip2(path, true)
    }

    fn _open_bzip2<P: AsRef<Path>>(path: P, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        let file = File::open(path)?;
        let mut decoder = bzip2::read::BzDecoder::new(file);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;

        if buf.len() < 1024 {
            return Err(Error::InvalidHeader);
        }

        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&buf[..1024]);
        let (header, endian_warning) = Header::decode_from_bytes_with_info(&header_bytes);

        let mut warnings = if permissive {
            header.validate_permissive().map_err(Error::InvalidHeaderDetailed)?
        } else {
            header.validate_detailed().map_err(Error::InvalidHeaderDetailed)?;
            Vec::new()
        };

        if let Some(msg) = endian_warning {
            warnings.push(msg.to_string());
        }

        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let ext_size = header.nsymbt as usize;

        if !permissive {
            if buf.len() != 1024 + ext_size + data_size {
                return Err(Error::FileSizeMismatch {
                    expected: 1024 + ext_size + data_size,
                    actual: buf.len(),
                });
            }
        } else if buf.len() != 1024 + ext_size + data_size {
            warnings.push(format!(
                "File size mismatch: expected {} bytes, got {}",
                1024 + ext_size + data_size,
                buf.len()
            ));
        }

        let ext_header = buf[1024..1024 + ext_size].to_vec();
        let data = buf[1024 + ext_size..].to_vec();

        let endian = header.detect_endian();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok((Self {
            header,
            ext_header,
            data,
            endian,
            shape,
        }, warnings))
    }
}

/// Bzip2 compressor backend for [`CompressedWriter`](crate::io::writer::CompressedWriter).
#[derive(Debug)]
pub struct Bzip2Compressor;

impl crate::io::writer::Compressor for Bzip2Compressor {
    fn compress(data: &[u8]) -> Result<Vec<u8>, Error> {
        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        std::io::Write::write_all(&mut encoder, data)?;
        Ok(encoder.finish()?)
    }
}

/// Bzip2-compressed MRC file writer.
///
/// Because bzip2 does not support random access, the entire file is buffered
/// in memory and compressed only on finalize.
/// For large volumes consider using [`Writer`](crate::Writer) instead.
pub type Bzip2Writer = crate::io::writer::CompressedWriter<Bzip2Compressor>;
