//! I/O module for reading and writing MRC files.

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
