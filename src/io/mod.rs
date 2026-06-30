//! I/O subsystem for reading and writing MRC files.
//!
//! This module provides multiple I/O strategies tailored to different use cases:
//!
//! | Type | Module | Description |
//! |------|--------|-------------|
//! | [`MrcReader`] | [`reader`] | Unified entry point that auto-detects compression (plain, gzip, bzip2) and dispatches to the correct backend. This is what [`crate::open`] returns. |
//! | [`Reader`] | [`buffered`] | In-memory buffered reader. Also provides [`Reader::open_gzip`] and [`Reader::open_bzip2`] for direct compressed reads. |
//! | [`MmapReader`] | [`mmap_reader`] | Memory-mapped reader. Lets the OS page data on demand; ideal for files too large to fit in RAM (requires the `mmap` feature). |
//! | [`Writer`] | [`writer`] | Direct file I/O writer. Writes blocks straight to disk and rewrites the header on [`Writer::finalize`]. Use [`WriterBuilder`] to construct. |
//! | [`MmapWriter`] | [`writer`] | Memory-mapped writer, built via [`WriterBuilder::finish_mmap`]. |
//! | [`GzipWriter`] / [`Bzip2Writer`] | [`gzip`] / [`bzip2`] | Compressed writers that buffer in memory and compress on finalize. |
//!
//! ## Choosing a reader
//!
//! * Use [`MrcReader`] / [`crate::open`] when you simply want to read a file and don't know
//!   whether it is compressed.
//! * Use [`Reader`] / [`Reader::open`] for plain files, or [`Reader::open_gzip`] /
//!   [`Reader::open_bzip2`] when you know the compression format.
//! * Use [`MmapReader`] when the file is very large or you only need to touch a small
//!   subset of the data.
//!
//! ## Choosing a writer
//!
//! * Use [`WriterBuilder`] / [`crate::create`] for normal uncompressed output.
//!   Call `.finish()` for a file-backed [`Writer`] or `.finish_mmap()` for a
//!   memory-mapped [`MmapWriter`].
//! * Use [`GzipWriter`] / [`Bzip2Writer`] when you need compressed output. Note that
//!   these buffer everything in RAM because compressed formats do not support random
//!   access.
//!
//! ## Shared internals
//!
//! [`reader_common`] contains helper traits and functions used across all reader
//! implementations, including the sealed [`VoxelSource`](crate::io::reader_common::VoxelSource)
//! trait that powers the generic slice/slab iterators.

pub mod buffered;
pub mod reader;
pub mod reader_common;
pub mod writer;

#[cfg(feature = "mmap")]
pub mod mmap_reader;

#[cfg(feature = "gzip")]
pub mod gzip;

#[cfg(feature = "bzip2")]
pub mod bzip2;
