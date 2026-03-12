//! MRC file writer

use crate::{Error, Header, RawHeader};
use alloc::string::ToString;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Seek, SeekFrom, Write};

/// MRC file writer
#[cfg(feature = "std")]
pub struct MrcWriter {
    file: File,
    header: Header,
    ext_header: Vec<u8>,
    data_offset: u64,
}

#[cfg(feature = "std")]
impl MrcWriter {
    /// Create a new MRC file
    pub fn create(path: impl AsRef<std::path::Path>, header: Header) -> Result<Self, Error> {
        Self::create_with_ext_header(path, header, &[])
    }
    
    /// Create a new MRC file with extended header
    pub fn create_with_ext_header(
        path: impl AsRef<std::path::Path>,
        header: Header,
        ext_header: &[u8],
    ) -> Result<Self, Error> {
        let mut file = File::create(path).map_err(|e| Error::Io(e.to_string()))?;
        
        // Write header
        let raw: RawHeader = header.clone().into();
        let header_bytes = bytemuck::bytes_of(&raw);
        file.write_all(header_bytes).map_err(|e| Error::Io(e.to_string()))?;
        
        // Write extended header
        if !ext_header.is_empty() {
            file.write_all(ext_header).map_err(|e| Error::Io(e.to_string()))?;
        }
        
        let data_offset = header.data_offset() as u64;
        
        Ok(Self {
            file,
            header,
            ext_header: ext_header.to_vec(),
            data_offset,
        })
    }
    
    /// Get the header
    pub fn header(&self) -> &Header {
        &self.header
    }
    
    /// Get mutable header reference
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }
    
    /// Write data to the file
    pub fn write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.header.data_size() {
            return Err(Error::BufferTooSmall {
                expected: self.header.data_size(),
                got: data.len(),
            });
        }
        
        self.file.seek(SeekFrom::Start(self.data_offset))
            .map_err(|e| Error::Io(e.to_string()))?;
        self.file.write_all(data).map_err(|e| Error::Io(e.to_string()))?;
        
        Ok(())
    }
    
    /// Update the header on disk
    pub fn flush_header(&mut self) -> Result<(), Error> {
        let raw: RawHeader = self.header.clone().into();
        let header_bytes = bytemuck::bytes_of(&raw);
        self.file.seek(SeekFrom::Start(0))
            .map_err(|e| Error::Io(e.to_string()))?;
        self.file.write_all(header_bytes).map_err(|e| Error::Io(e.to_string()))?;
        Ok(())
    }
}