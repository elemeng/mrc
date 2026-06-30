//! Gzip-compressed MRC file reader and writer.
//!
//! Because gzip does not support random access, the writer buffers the entire
//! file in memory and compresses on [`CompressedWriter::finalize`]. This matches
//! the behaviour of the reference Python `mrcfile` library.

use crate::Error;

use std::fs::File;
use std::path::Path;

impl crate::Reader {
    /// Open a gzip-compressed MRC file.
    ///
    /// The entire file is decompressed into memory on open.
    pub fn open_gzip<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::_open_gzip(path, false).map(|(r, _)| r)
    }

    /// Open a gzip-compressed MRC file in **permissive** mode.
    pub fn open_gzip_permissive<P: AsRef<Path>>(
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
///
/// This type is `#[doc(hidden)]` — use the [`GzipWriter`](crate::GzipWriter) type alias.
#[doc(hidden)]
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
///
/// Construct via [`WriterBuilder::finish_gzip`](crate::WriterBuilder::finish_gzip)
pub type GzipWriter = crate::io::writer::CompressedWriter<GzipCompressor>;
