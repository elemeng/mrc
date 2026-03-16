//! MRC file reader with iterator-centric API

use crate::decode::{decode_f32, decode_i16};
use crate::iter::{BlockIter, SliceIter, SlabIter};
use crate::{Error, FileEndian, Header, Mode, VolumeShape};

use alloc::vec::Vec;

pub struct Reader {
    header: Header,
    data: Vec<u8>,
    endian: FileEndian,
    shape: VolumeShape,
}

impl Reader {
    pub fn open(path: &str) -> Result<Self, Error> {
        #[cfg(feature = "std")]
        {
            use std::fs::File;
            use std::io::Read;

            let mut file = File::open(path).map_err(|_| Error::Io)?;

            let mut header_bytes = [0u8; 1024];
            file.read_exact(&mut header_bytes).map_err(|_| Error::Io)?;

            let header = Header::decode_from_bytes(&header_bytes);

            if !header.validate() {
                return Err(Error::InvalidHeader);
            }

            let _data_offset = header.data_offset();
            let data_size = header.data_size();

            let ext_size = header.nsymbt as usize;
            if ext_size > 0 {
                let mut ext_data = alloc::vec![0u8; ext_size];
                file.read_exact(&mut ext_data).map_err(|_| Error::Io)?;
            }

            let mut data = alloc::vec![0u8; data_size];
            file.read_exact(&mut data).map_err(|_| Error::Io)?;

            let endian = header.detect_endian();
            let shape = VolumeShape::new(
                header.nx as usize,
                header.ny as usize,
                header.nz as usize,
            );

            Ok(Self {
                header,
                data,
                endian,
                shape,
            })
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = path;
            Err(Error::Io)
        }
    }

    pub fn shape(&self) -> VolumeShape {
        self.shape
    }

    pub fn mode(&self) -> Mode {
        Mode::from_i32(self.header.mode).unwrap_or(Mode::Float32)
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn slices<T>(&self) -> SliceIter<T> {
        SliceIter::new(self, self.shape)
    }

    pub fn slabs<T>(&self, k: usize) -> SlabIter<T> {
        SlabIter::new(self, self.shape, k)
    }

    pub fn blocks<T>(&self, chunk_shape: [usize; 3]) -> BlockIter<T> {
        BlockIter::new(self, self.shape, chunk_shape)
    }

    pub fn read_voxels(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<Vec<u8>, Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        let bytes_per_voxel = match self.mode() {
            Mode::Float32 => 4,
            Mode::Int16 => 2,
            Mode::Uint16 => 2,
            Mode::Int8 => 1,
            _ => return Err(Error::UnsupportedMode),
        };

        let start_byte = (ox + oy * nx + oz * nx * ny) * bytes_per_voxel;
        let end_byte = start_byte + sx * sy * sz * bytes_per_voxel;

        if end_byte > self.data.len() {
            return Err(Error::BoundsError);
        }

        Ok(self.data[start_byte..end_byte].to_vec())
    }

    pub fn decode_block_f32(&self, bytes: &[u8]) -> Result<Vec<f32>, Error> {
        if self.mode() != Mode::Float32 {
            return Err(Error::UnsupportedMode);
        }
        let mut result = alloc::vec![0f32; bytes.len() / 4];
        for i in 0..result.len() {
            result[i] = decode_f32(bytes, i * 4, self.endian);
        }
        Ok(result)
    }

    pub fn decode_block_i16(&self, bytes: &[u8]) -> Result<Vec<i16>, Error> {
        if self.mode() != Mode::Int16 {
            return Err(Error::UnsupportedMode);
        }
        let mut result = alloc::vec![0i16; bytes.len() / 2];
        for i in 0..result.len() {
            result[i] = decode_i16(bytes, i * 2, self.endian);
        }
        Ok(result)
    }

    pub fn decode_block_generic<T: Clone>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        match self.mode() {
            Mode::Float32 => {
                let f32_data = self.decode_block_f32(bytes)?;
                unsafe {
                    let ptr = f32_data.as_ptr() as *const T;
                    let slice = core::slice::from_raw_parts(ptr, f32_data.len());
                    Ok(slice.to_vec())
                }
            }
            Mode::Int16 => {
                let i16_data = self.decode_block_i16(bytes)?;
                unsafe {
                    let ptr = i16_data.as_ptr() as *const T;
                    let slice = core::slice::from_raw_parts(ptr, i16_data.len());
                    Ok(slice.to_vec())
                }
            }
            _ => Err(Error::UnsupportedMode),
        }
    }
}