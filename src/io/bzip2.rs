//! Bzip2-compressed MRC file reader and writer.
//!
//! Because bzip2 does not support random access, the writer buffers the entire
//! file in memory and compresses on [`CompressedWriter::finalize`]. This matches
//! the behaviour of the reference Python `mrcfile` library.

use crate::Error;

use std::fs::File;
use std::path::Path;

impl crate::Reader {
    /// Open a bzip2-compressed MRC file.
    ///
    /// The entire file is decompressed into memory on open.
    pub fn open_bzip2<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::_open_bzip2(path, false).map(|(r, _)| r)
    }

    /// Open a bzip2-compressed MRC file in **permissive** mode.
    pub fn open_bzip2_permissive<P: AsRef<Path>>(
        path: P,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_bzip2(path, true)
    }

    fn _open_bzip2<P: AsRef<Path>>(
        path: P,
        permissive: bool,
    ) -> Result<(Self, Vec<String>), Error> {
        let file = File::open(path)?;
        let decoder = bzip2::read::BzDecoder::new(file);
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

/// Bzip2 compressor backend for [`CompressedWriter`](crate::io::writer::CompressedWriter).
///
/// This type is `#[doc(hidden)]` — use the [`Bzip2Writer`](crate::Bzip2Writer) type alias.
#[doc(hidden)]
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
///
/// Construct via [`WriterBuilder::finish_bzip2`](crate::WriterBuilder::finish_bzip2)
pub type Bzip2Writer = crate::io::writer::CompressedWriter<Bzip2Compressor>;
