//! MRC file writer

use crate::{Error, Header, Mode, RawHeader};
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Seek, SeekFrom, Write};

/// Builder for creating MRC files
#[cfg(feature = "std")]
pub struct MrcWriterBuilder {
    dimensions: [usize; 3],
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
            dimensions: [1, 1, 1],
            mode: Mode::Float32,
            voxel_size: [1.0, 1.0, 1.0],
            origin: [0.0, 0.0, 0.0],
            cell_angles: [90.0, 90.0, 90.0],
            ext_header: Vec::new(),
            data: None,
        }
    }

    /// Set the dimensions (nx, ny, nz)
    pub fn dimensions(mut self, nx: usize, ny: usize, nz: usize) -> Self {
        self.dimensions = [nx, ny, nz];
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
    ///
    /// # Errors
    /// Returns `Error::BufferTooSmall` or `Error::InvalidDimensions` if the provided data
    /// size doesn't match the expected size based on dimensions and mode.
    pub fn write(self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        // Calculate expected data size
        let voxel_count = self.dimensions[0]
            .checked_mul(self.dimensions[1])
            .and_then(|v| v.checked_mul(self.dimensions[2]))
            .ok_or(Error::InvalidDimensions)?;

        let expected_data_size = if self.mode == Mode::Packed4Bit {
            voxel_count.div_ceil(2)
        } else {
            voxel_count
                .checked_mul(self.mode.byte_size())
                .ok_or(Error::InvalidDimensions)?
        };

        // Validate data size if provided
        if let Some(ref data) = self.data {
            if data.len() != expected_data_size {
                return Err(Error::BufferTooSmall {
                    expected: expected_data_size,
                    got: data.len(),
                });
            }
        }

        let mut header = Header::new();
        header.set_dimensions(self.dimensions[0], self.dimensions[1], self.dimensions[2]);
        header.set_mode(self.mode);
        header.set_cell_dimensions(
            self.voxel_size[0] * self.dimensions[0] as f32,
            self.voxel_size[1] * self.dimensions[1] as f32,
            self.voxel_size[2] * self.dimensions[2] as f32,
        );
        header.raw.alpha = self.cell_angles[0];
        header.raw.beta = self.cell_angles[1];
        header.raw.gamma = self.cell_angles[2];
        header.set_origin(self.origin[0], self.origin[1], self.origin[2]);
        header.set_nsymbt(self.ext_header.len());

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
        let mut file = File::create(path).map_err(Error::from)?;

        // Write header
        let raw: RawHeader = header.clone().into();
        let header_bytes = bytemuck::bytes_of(&raw);
        file.write_all(header_bytes).map_err(Error::from)?;

        // Write extended header
        if !ext_header.is_empty() {
            file.write_all(ext_header).map_err(Error::from)?;
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

        self.file
            .seek(SeekFrom::Start(self.data_offset))
            .map_err(Error::from)?;
        self.file.write_all(data).map_err(Error::from)?;

        Ok(())
    }

    /// Update the header on disk
    pub fn flush_header(&mut self) -> Result<(), Error> {
        let raw: RawHeader = self.header.clone().into();
        let header_bytes = bytemuck::bytes_of(&raw);
        self.file.seek(SeekFrom::Start(0)).map_err(Error::from)?;
        self.file.write_all(header_bytes).map_err(Error::from)?;
        Ok(())
    }
}

use super::traits::MrcSink;

impl MrcSink for MrcWriter {
    fn write_header(&mut self, header: &Header) -> Result<(), Error> {
        let raw: RawHeader = header.clone().into();
        let header_bytes = bytemuck::bytes_of(&raw);
        self.file.write_all(header_bytes).map_err(Error::from)
    }

    fn write_data_bytes(&mut self, header: &Header, data: &[u8]) -> Result<(), Error> {
        if data.len() != header.data_size() {
            return Err(Error::BufferTooSmall {
                expected: header.data_size(),
                got: data.len(),
            });
        }
        self.file.write_all(data).map_err(Error::from)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.file.write_all(buf).map_err(Error::from)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.file.flush().map_err(Error::from)
    }
}