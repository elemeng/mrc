//! Gzip-compressed MRC file reader and writer.
//!
//! Because gzip does not support random access, the writer buffers the entire
//! file in memory and compresses on [`CompressedWriter::finalize`]. This matches
//! the behaviour of the reference Python `mrcfile` library.

use crate::Error;

use std::fs::File;
use std::path::Path;
use std::vec::Vec;

/// Gzip-compressed MRC file reader.
///
/// This is a thin newtype wrapper around [`Reader`](crate::Reader). All
/// [`Reader`](crate::Reader) methods are available via [`Deref`](std::ops::Deref).
#[derive(Debug)]
pub struct GzipReader(pub crate::Reader);

impl std::ops::Deref for GzipReader {
    type Target = crate::Reader;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for GzipReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl GzipReader {
    /// Open a gzip-compressed MRC file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        crate::Reader::open_gzip(path).map(Self)
    }

    /// Open a gzip-compressed MRC file in **permissive** mode.
    pub fn open_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        crate::Reader::open_gzip_permissive(path).map(|(r, w)| (Self(r), w))
    }
}

impl crate::Reader {
    /// Open a gzip-compressed MRC file.
    pub(crate) fn open_gzip<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::_open_gzip(path, false).map(|(r, _)| r)
    }

    /// Open a gzip-compressed MRC file in **permissive** mode.
    pub(crate) fn open_gzip_permissive<P: AsRef<Path>>(
        path: P,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_gzip(path, true)
    }

    fn _open_gzip<P: AsRef<Path>>(path: P, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        let file = File::open(path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let d = crate::io::reader_common::open_compressed(decoder, permissive)?;
        Ok((
            Self {
                header: d.header,
                ext_header: d.ext_header,
                data: d.data,
                endian: d.endian,
                shape: d.shape,
            },
            d.warnings,
        ))
    }
}

/// Gzip compressor backend for [`CompressedWriter`](crate::io::writer::CompressedWriter).
#[derive(Debug)]
pub struct GzipCompressor;

impl crate::io::writer::Compressor for GzipCompressor {
    fn compress(data: &[u8]) -> Result<Vec<u8>, Error> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, data)?;
        Ok(encoder.finish()?)
    }
}

/// Gzip-compressed MRC file writer.
///
/// Because gzip does not support random access, the entire file is buffered
/// in memory and compressed only on finalize.
/// For large volumes consider using [`Writer`](crate::Writer) instead.
pub type GzipWriter = crate::io::writer::CompressedWriter<GzipCompressor>;
