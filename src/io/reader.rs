//! MRC file reader

use crate::{Error, Header, Mode, Volume, Encoding, Voxel, VolumeData, ExtendedHeader};
use alloc::string::ToString;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Read, Seek, SeekFrom};

/// MRC file reader
#[cfg(feature = "std")]
pub struct MrcReader {
    file: File,
    header: Header,
    ext_header: Vec<u8>,
    data_offset: u64,
    data_size: usize,
}

#[cfg(feature = "std")]
impl MrcReader {
    /// Open an MRC file for reading
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let mut file = File::open(path).map_err(|e| Error::Io(e.to_string()))?;
        
        // Read header
        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes).map_err(|e| Error::Io(e.to_string()))?;
        
        let raw_header: crate::RawHeader = *bytemuck::from_bytes(&header_bytes);
        let header = Header::try_from(raw_header)?;
        
        // Read extended header
        let ext_header_size = header.nsymbt;
        let mut ext_header = alloc::vec![0u8; ext_header_size];
        if ext_header_size > 0 {
            file.read_exact(&mut ext_header).map_err(|e| Error::Io(e.to_string()))?;
        }
        
        let data_offset = header.data_offset() as u64;
        let data_size = header.data_size();
        
        Ok(Self {
            file,
            header,
            ext_header,
            data_offset,
            data_size,
        })
    }
    
    /// Get the validated header
    pub fn header(&self) -> &Header {
        &self.header
    }
    
    /// Get the extended header bytes
    pub fn ext_header(&self) -> &[u8] {
        &self.ext_header
    }
    
    /// Get parsed extended header
    pub fn ext_header_parsed(&self) -> ExtendedHeader {
        ExtendedHeader::from_bytes(&self.header.exttyp, self.ext_header.clone())
    }
    
    /// Get the mode
    pub fn mode(&self) -> Mode {
        self.header.mode
    }
    
    /// Get dimensions
    pub fn dimensions(&self) -> (usize, usize, usize) {
        self.header.dimensions()
    }
    
    /// Read all data into a vector
    pub fn read_data(&mut self) -> Result<Vec<u8>, Error> {
        let mut data = alloc::vec![0u8; self.data_size];
        self.file.seek(SeekFrom::Start(self.data_offset))
            .map_err(|e| Error::Io(e.to_string()))?;
        self.file.read_exact(&mut data)
            .map_err(|e| Error::Io(e.to_string()))?;
        Ok(data)
    }
    
    /// Read volume with compile-time type checking
    ///
    /// # Type Parameters
    /// - `T`: Voxel type (must implement Voxel + Encoding)
    ///
    /// # Errors
    /// Returns `Error::TypeMismatch` if the file mode doesn't match T::MODE
    pub fn read_volume<T: Voxel + Encoding>(&mut self) -> Result<Volume<T, Vec<u8>>, Error> {
        // Verify mode matches
        if self.header.mode != T::MODE {
            return Err(Error::TypeMismatch);
        }
        
        let data = self.read_data()?;
        Volume::new(self.header.clone(), data)
    }
    
    /// Read volume with dynamic type dispatch
    ///
    /// Returns a `VolumeData` enum containing the appropriate typed volume.
    pub fn read(&mut self) -> Result<VolumeData, Error> {
        let data = self.read_data()?;
        VolumeData::from_bytes(self.header.clone(), data)
    }
}
