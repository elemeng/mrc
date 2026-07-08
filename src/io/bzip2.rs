//! Bzip2-compressed MRC file reader and writer.
//!
//! Because bzip2 does not support random access, the writer buffers the entire
//! file in memory and compresses on [`CompressedWriter::finalize`]. This matches
//! the behaviour of the reference Python `mrcfile` library.
//!
//! The reader applies a safety limit of [`DEFAULT_MAX_DECOMPRESSED_BYTES`]
//! (256 GiB) during decompression, enforced before the header is parsed.
//! Use [`open_bzip2_with_limit`](crate::Reader::open_bzip2_with_limit) to
//! override.
//!
//! [`DEFAULT_MAX_DECOMPRESSED_BYTES`]: crate::DEFAULT_MAX_DECOMPRESSED_BYTES

use crate::Error;

use std::fs::File;
use std::path::Path;

impl crate::Reader {
    /// Open a bzip2-compressed MRC file.
    ///
    /// Requires the `bzip2` feature (disabled by default).
    /// The file is decompressed into memory with a safety limit of
    /// [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB). To set a custom limit,
    /// use [`open_bzip2_with_limit`](Self::open_bzip2_with_limit).
    ///
    /// [`DEFAULT_MAX_DECOMPRESSED_BYTES`]: crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES
    pub fn open_bzip2<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::open_bzip2_with_limit(
            path,
            crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
        )
    }

    /// Open a bzip2-compressed MRC file with a custom decompression byte limit.
    ///
    /// The decompressed stream is capped at `max_bytes` before the header is
    /// parsed. If the stream exceeds this limit, a decompression-bomb error
    /// is returned. Use the default [`open_bzip2`](Self::open_bzip2) for the
    /// built-in 256 GiB safety limit.
    pub fn open_bzip2_with_limit<P: AsRef<Path>>(path: P, max_bytes: u64) -> Result<Self, Error> {
        Self::_open_bzip2(path, false, max_bytes).map(|(r, _)| r)
    }

    /// Open a bzip2-compressed MRC file in **permissive** mode.
    pub fn open_bzip2_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        Self::_open_bzip2(
            path,
            true,
            crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
        )
    }

    fn _open_bzip2<P: AsRef<Path>>(
        path: P,
        permissive: bool,
        max_bytes: u64,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_bzip2_file(File::open(path)?, permissive, max_bytes)
    }

    /// Internal: parse from an already-opened file handle (avoids redundant
    /// `File::open` when the caller has already peeked at magic bytes).
    pub(crate) fn _open_bzip2_file(
        file: File,
        permissive: bool,
        max_bytes: u64,
    ) -> Result<(Self, Vec<String>), Error> {
        let decoder = bzip2::read::BzDecoder::new(file);
        let d = crate::io::reader_common::open_compressed(decoder, permissive, max_bytes)?;
        Self::_from_decompressed(d)
    }
}

/// Bzip2 compressor backend.
///
/// This type is `#[doc(hidden)]`.
#[doc(hidden)]
#[derive(Debug)]
pub struct Bzip2Compressor;

impl crate::io::writer::Compressor for Bzip2Compressor {
    fn compress(data: &[u8], level: crate::io::writer::Compression) -> Result<Vec<u8>, Error> {
        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), level.to_bzip2());
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
