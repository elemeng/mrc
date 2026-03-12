//! MRC file writer

use crate::{Error, Header, RawHeader, Mode};
use alloc::string::ToString;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Seek, SeekFrom, Write};

/// Builder for creating MRC files
#[cfg(feature = "std")]
pub struct MrcWriterBuilder {
    shape: [usize; 3],
    mode: Mode,
    voxel_size: [f32; 3],
    origin: [f32; 3],
    cell_angles: [f32; 3],
    ext_header: Vec<u8>,
    data: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
impl MrcWriterBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            shape: [1, 1, 1],
            mode: Mode::Float32,
            voxel_size: [1.0, 1.0, 1.0],
            origin: [0.0, 0.0, 0.0],
            cell_angles: [90.0, 90.0, 90.0],
            ext_header: Vec::new(),
            data: None,
        }
    }
    
    /// Set the shape (nx, ny, nz)
    pub fn shape(mut self, nx: usize, ny: usize, nz: usize) -> Self {
        self.shape = [nx, ny, nz];
        self
    }
    
    /// Set the mode
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
    
    /// Set voxel size in Angstroms
    pub fn voxel_size(mut self, dx: f32, dy: f32, dz: f32) -> Self {
        self.voxel_size = [dx, dy, dz];
        self
    }
    
    /// Set origin in Angstroms
    pub fn origin(mut self, x: f32, y: f32, z: f32) -> Self {
        self.origin = [x, y, z];
        self
    }
    
    /// Set cell angles in degrees
    pub fn cell_angles(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
        self.cell_angles = [alpha, beta, gamma];
        self
    }
    
    /// Set extended header bytes
    pub fn ext_header(mut self, data: Vec<u8>) -> Self {
        self.ext_header = data;
        self
    }
    
    /// Set the data bytes
    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = Some(data);
        self
    }
    
    /// Build the writer and write to file
    pub fn write(self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let mut header = Header::default();
        header.nx = self.shape[0];
        header.ny = self.shape[1];
        header.nz = self.shape[2];
        header.mode = self.mode;
        header.xlen = self.voxel_size[0] * self.shape[0] as f32;
        header.ylen = self.voxel_size[1] * self.shape[1] as f32;
        header.zlen = self.voxel_size[2] * self.shape[2] as f32;
        header.alpha = self.cell_angles[0];
        header.beta = self.cell_angles[1];
        header.gamma = self.cell_angles[2];
        header.xorigin = self.origin[0];
        header.yorigin = self.origin[1];
        header.zorigin = self.origin[2];
        header.nsymbt = self.ext_header.len();
        
        let mut writer = if self.ext_header.is_empty() {
            MrcWriter::create(path, header)?
        } else {
            MrcWriter::create_with_ext_header(path, header, &self.ext_header)?
        };
        
        if let Some(data) = self.data {
            writer.write_data(&data)?;
        }
        
        Ok(())
    }
}

#[cfg(feature = "std")]
impl Default for MrcWriterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// MRC file writer
#[cfg(feature = "std")]
pub struct MrcWriter {
    file: File,
    header: Header,
    /// Extended header bytes (stored for reference)
    _ext_header: Vec<u8>,
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
            _ext_header: ext_header.to_vec(),
            data_offset,
        })
    }
    
    /// Create a builder for writing MRC files
    pub fn builder() -> MrcWriterBuilder {
        MrcWriterBuilder::new()
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
