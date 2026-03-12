//! MRC file reader

use crate::{Error, Header};
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
    
    /// Read all data into a vector
    pub fn read_data(&mut self) -> Result<Vec<u8>, Error> {
        let mut data = alloc::vec![0u8; self.data_size];
        self.file.seek(SeekFrom::Start(self.data_offset))
            .map_err(|e| Error::Io(e.to_string()))?;
        self.file.read_exact(&mut data)
            .map_err(|e| Error::Io(e.to_string()))?;
        Ok(data)
    }
}