//! IO traits for abstracting over different data sources
//!
//! These traits enable reading from and writing to various sources:
//! - Files
//! - In-memory buffers (Cursor<Vec<u8>>)
//! - Network streams
//! - Custom implementations for testing

use crate::access::Volume;
use crate::core::Error;
use crate::header::Header;
use crate::voxel::{Encoding, Voxel, validate_mode};
use alloc::vec;
use alloc::vec::Vec;

/// Trait for sources that can provide MRC data
///
/// Implement this trait for types that can read MRC headers and voxel data.
/// This enables reading from files, memory buffers, network streams, etc.
///
/// # Example
/// ```no_run
/// use mrc::io::MrcSource;
/// use mrc::MrcReader;
///
/// // Read from file
/// let mut reader = MrcReader::open("data.mrc").unwrap();
/// let header = reader.read_header().unwrap();
/// ```
pub trait MrcSource {
    /// Read the MRC header
    ///
    /// # Errors
    /// Returns `Error::Io` if reading fails, or `Error::InvalidHeader` if the header is malformed.
    fn read_header(&mut self) -> Result<Header, Error>;

    /// Read raw voxel data bytes
    ///
    /// # Arguments
    /// * `header` - The header containing data size information
    ///
    /// # Errors
    /// Returns `Error::Io` if reading fails, or `Error::BufferTooSmall` if not enough data.
    fn read_data_bytes(&mut self, header: &Header) -> Result<Vec<u8>, Error>;

    /// Read extended header bytes
    ///
    /// # Arguments
    /// * `header` - The header containing extended header size
    ///
    /// # Errors
    /// Returns `Error::Io` if reading fails.
    fn read_extended_header(&mut self, header: &Header) -> Result<Vec<u8>, Error> {
        let size = header.nsymbt();
        if size == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0u8; size];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Read exact number of bytes
    ///
    /// # Errors
    /// Returns `Error::Io` if reading fails or EOF is reached early.
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;

    /// Read a typed volume
    ///
    /// # Type Parameters
    /// * `T` - The voxel type, must implement `Voxel + Encoding`
    ///
    /// # Errors
    /// Returns `Error::TypeMismatch` if the file mode doesn't match T::MODE.
    fn read_volume<T: Voxel + Encoding>(&mut self) -> Result<Volume<T, Vec<u8>>, Error> {
        let header = self.read_header()?;
        validate_mode::<T>(header.mode())?;
        let data = self.read_data_bytes(&header)?;
        Volume::new(header, data)
    }
}

/// Trait for sinks that can receive MRC data
///
/// Implement this trait for types that can write MRC headers and voxel data.
/// This enables writing to files, memory buffers, network streams, etc.
pub trait MrcSink {
    /// Write the MRC header
    ///
    /// # Errors
    /// Returns `Error::Io` if writing fails.
    fn write_header(&mut self, header: &Header) -> Result<(), Error>;

    /// Write raw voxel data bytes
    ///
    /// # Errors
    /// Returns `Error::Io` if writing fails, or `Error::BufferTooSmall` if data doesn't match header.
    fn write_data_bytes(&mut self, header: &Header, data: &[u8]) -> Result<(), Error>;

    /// Write extended header bytes
    ///
    /// # Errors
    /// Returns `Error::Io` if writing fails.
    fn write_extended_header(&mut self, data: &[u8]) -> Result<(), Error> {
        self.write_all(data)
    }

    /// Write all bytes
    ///
    /// # Errors
    /// Returns `Error::Io` if writing fails.
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error>;

    /// Flush any buffered data
    ///
    /// # Errors
    /// Returns `Error::Io` if flushing fails.
    fn flush(&mut self) -> Result<(), Error>;

    /// Write a typed volume
    ///
    /// # Type Parameters
    /// * `T` - The voxel type, must implement `Voxel + Encoding`
    ///
    /// # Errors
    /// Returns `Error::TypeMismatch` if the volume mode doesn't match T::MODE.
    fn write_volume<T: Voxel + Encoding>(
        &mut self,
        volume: &Volume<T, Vec<u8>>,
    ) -> Result<(), Error> {
        let header = volume.header();
        validate_mode::<T>(header.mode())?;
        self.write_header(header)?;
        self.write_data_bytes(header, volume.as_bytes())?;
        self.flush()
    }
}

// Async-capable IO traits (for future async support)

// Note: These will be added when async support is fully implemented.

// For now, use the synchronous `MrcSource` and `MrcSink` traits.


