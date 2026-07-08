//! I/O subsystem for reading and writing MRC files.
//!
//! The [`Reader`] auto-selects between memory-mapped and buffered I/O.
//! The [`Writer`] supports file, mmap, gzip, and bzip2 output through a single type.
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Reader`] | Auto-selects mmap (zero-copy, large files) or buffered (in-memory, small files). Auto-detects compression. |
//! | [`Writer`] / [`WriterBuilder`] | File, mmap, gzip, or bzip2 output via one builder. |
//!
//! ## Reading
//!
//! * [`Reader::open`] — auto-detects compression, uses mmap when possible.
//! * [`Reader::from_reader`] / [`Reader::from_bytes`] — from memory or streams.
//!
//! ## Writing
//!
//! * [`WriterBuilder`] / [`crate::create`] — configure and create a writer.
//!   Use `.finish()` for files, `.finish_gzip()` for compressed output.

pub mod reader;
pub mod reader_common;
pub mod writer;

#[cfg(feature = "gzip")]
pub mod gzip;

#[cfg(feature = "bzip2")]
pub mod bzip2;
