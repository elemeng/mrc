//! Gzip-compressed MRC file reader.
//!
//! The reader applies a safety limit of [`DEFAULT_MAX_DECOMPRESSED_BYTES`]
//! (256 GiB) during decompression, enforced before the header is parsed.
//! Use [`open_gzip_with_limit`](crate::Reader::open_gzip_with_limit) to
//! override.
//!
//! The gzip writer is built via [`WriterBuilder::finish_gzip`](crate::WriterBuilder::finish_gzip).
//!
//! [`DEFAULT_MAX_DECOMPRESSED_BYTES`]: crate::DEFAULT_MAX_DECOMPRESSED_BYTES

use crate::Error;

use std::fs::File;
use std::path::Path;

impl crate::Reader {
    /// Open a gzip-compressed MRC file.
    ///
    /// Requires the `gzip` feature (enabled by default).
    /// The file is decompressed into memory with a safety limit of
    /// [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB). To set a custom limit,
    /// use [`open_gzip_with_limit`](Self::open_gzip_with_limit).
    ///
    /// [`DEFAULT_MAX_DECOMPRESSED_BYTES`]: crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES
    pub fn open_gzip<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::open_gzip_with_limit(
            path,
            crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
        )
    }

    /// Open a gzip-compressed MRC file with a custom decompression byte limit.
    ///
    /// The decompressed stream is capped at `max_bytes` before the header is
    /// parsed. If the stream exceeds this limit, a decompression-bomb error
    /// is returned. Use the default [`open_gzip`](Self::open_gzip) for the
    /// built-in 256 GiB safety limit.
    pub fn open_gzip_with_limit<P: AsRef<Path>>(path: P, max_bytes: u64) -> Result<Self, Error> {
        Self::_open_gzip(path, false, max_bytes).map(|(r, _)| r)
    }

    /// Open a gzip-compressed MRC file in **permissive** mode.
    pub fn open_gzip_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        Self::_open_gzip(
            path,
            true,
            crate::io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES,
        )
    }

    fn _open_gzip<P: AsRef<Path>>(
        path: P,
        permissive: bool,
        max_bytes: u64,
    ) -> Result<(Self, Vec<String>), Error> {
        Self::_open_gzip_file(File::open(path)?, permissive, max_bytes)
    }

    /// Internal: parse from an already-opened file handle (avoids redundant
    /// `File::open` when the caller has already peeked at magic bytes).
    pub(crate) fn _open_gzip_file(
        file: File,
        permissive: bool,
        max_bytes: u64,
    ) -> Result<(Self, Vec<String>), Error> {
        let decoder = flate2::read::GzDecoder::new(file);
        let d = crate::io::reader_common::open_compressed(decoder, permissive, max_bytes)?;
        Self::_from_decompressed(d)
    }
}
