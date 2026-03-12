//! IO traits for abstracting over different data sources

use crate::access::Volume;
use crate::core::Error;
use crate::header::Header;
use crate::voxel::{Encoding, Voxel, validate_mode};
use alloc::vec;
use alloc::vec::Vec;

/// Trait for sources that can provide MRC data
pub trait MrcSource {
    /// Read the MRC header
    fn read_header(&mut self) -> Result<Header, Error>;

    /// Read raw voxel data bytes
    fn read_data_bytes(&mut self, header: &Header) -> Result<Vec<u8>, Error>;

    /// Read extended header bytes
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
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;

    /// Read a typed volume
    fn read_volume<T: Voxel + Encoding>(&mut self) -> Result<Volume<T, Vec<u8>>, Error> {
        let header = self.read_header()?;
        validate_mode::<T>(header.mode())?;
        let data = self.read_data_bytes(&header)?;
        Volume::new(header, data)
    }
}

/// Trait for sinks that can receive MRC data
pub trait MrcSink {
    /// Write the MRC header
    fn write_header(&mut self, header: &Header) -> Result<(), Error>;

    /// Write raw voxel data bytes
    fn write_data_bytes(&mut self, header: &Header, data: &[u8]) -> Result<(), Error>;

    /// Write extended header bytes
    fn write_extended_header(&mut self, data: &[u8]) -> Result<(), Error> {
        self.write_all(data)
    }

    /// Write all bytes
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error>;

    /// Flush any buffered data
    fn flush(&mut self) -> Result<(), Error>;

    /// Write a typed volume
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
