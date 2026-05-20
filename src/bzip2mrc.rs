//! Bzip2-compressed MRC file reader and writer.
//!
//! Because bzip2 does not support random access, the writer buffers the entire
//! file in memory and compresses on [`Bzip2Writer::finalize`]. This matches the
//! behaviour of the reference Python `mrcfile` library.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::codec::decode_slice;
use crate::engine::endian::FileEndian;
use crate::{Error, Header, Mode};
use crate::mode::Voxel;

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Bzip2-compressed MRC file reader.
///
/// The entire file is decompressed into memory on open, after which the API
/// is identical to [`Reader`](crate::Reader).
#[derive(Debug)]
pub struct Bzip2Reader {
    header: Header,
    ext_header: Vec<u8>,
    data: Vec<u8>,
    endian: FileEndian,
    shape: VolumeShape,
}

impl Bzip2Reader {
    /// Open a bzip2-compressed MRC file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let file = File::open(path)?;
        let mut decoder = bzip2::read::BzDecoder::new(file);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;

        if buf.len() < 1024 {
            return Err(Error::InvalidHeader);
        }

        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&buf[..1024]);
        let header = Header::decode_from_bytes(&header_bytes);
        header.validate_detailed()?;

        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let ext_size = header.nsymbt as usize;

        if buf.len() != 1024 + ext_size + data_size {
            return Err(Error::FileSizeMismatch {
                expected: 1024 + ext_size + data_size,
                actual: buf.len(),
            });
        }

        let ext_header = buf[1024..1024 + ext_size].to_vec();
        let data = buf[1024 + ext_size..].to_vec();

        let endian = header.detect_endian();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            header,
            ext_header,
            data,
            endian,
            shape,
        })
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

    pub fn ext_header_bytes(&self) -> &[u8] {
        &self.ext_header
    }

    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        let [nx, ny, nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = offset;
        let [sx, sy, sz] = shape;

        if ox + sx > nx || oy + sy > ny || oz + sz > nz {
            return Err(Error::BoundsError);
        }

        if self.mode() == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }

        let linear = self.shape.checked_linear_index(offset).ok_or(Error::BoundsError)?;
        let start_byte = linear
            .checked_mul(self.mode().byte_size())
            .ok_or(Error::BoundsError)?;
        let count = sx.checked_mul(sy).and_then(|v| v.checked_mul(sz))
            .ok_or(Error::BoundsError)?;
        let byte_len = self.mode().byte_size_for_count(count);
        let end_byte = start_byte.checked_add(byte_len).ok_or(Error::BoundsError)?;

        if end_byte > self.data.len() {
            return Err(Error::BoundsError);
        }

        Ok(self.data[start_byte..end_byte].to_vec())
    }

    pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(VoxelBlock { offset, shape, data })
    }

    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }
        if self.endian == FileEndian::native() {
            let n = bytes.len() / T::BYTE_SIZE;
            let mut result = Vec::with_capacity(n);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    result.as_mut_ptr() as *mut u8,
                    bytes.len(),
                );
                result.set_len(n);
            }
            Ok(result)
        } else {
            Ok(decode_slice(bytes, self.endian))
        }
    }

    pub fn slices_f32(&self) -> Result<Box<dyn Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + '_>, Error> {
        use crate::engine::block::VoxelBlock;
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        match self.mode() {
            Mode::Float32 => Ok(Box::new((0..nz).map(move |z| {
                let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
                let data = self.decode_block::<f32>(&bytes)?;
                Ok(VoxelBlock { offset: [0, 0, z], shape: [nx, ny, 1], data })
            }))),
            Mode::Int16 => Ok(Box::new((0..nz).map(move |z| {
                let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
                let data = self.decode_block::<i16>(&bytes)?;
                let data = crate::engine::convert::convert_i16_slice_to_f32(&data);
                Ok(VoxelBlock { offset: [0, 0, z], shape: [nx, ny, 1], data })
            }))),
            Mode::Uint16 => Ok(Box::new((0..nz).map(move |z| {
                let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
                let data = self.decode_block::<u16>(&bytes)?;
                let data = crate::engine::convert::convert_u16_slice_to_f32(&data);
                Ok(VoxelBlock { offset: [0, 0, z], shape: [nx, ny, 1], data })
            }))),
            Mode::Int8 => Ok(Box::new((0..nz).map(move |z| {
                let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
                let data = self.decode_block::<i8>(&bytes)?;
                let data = crate::engine::convert::convert_i8_slice_to_f32(&data);
                Ok(VoxelBlock { offset: [0, 0, z], shape: [nx, ny, 1], data })
            }))),
            _ => Err(Error::UnsupportedMode),
        }
    }

    pub fn slices_mode0(
        &self,
        interp: crate::mode::M0Interpretation,
    ) -> impl Iterator<Item = Result<VoxelBlock<f32>, Error>> + '_ {
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        (0..nz).map(move |z| {
            if self.mode() != Mode::Int8 {
                return Err(Error::ModeMismatch {
                    file_mode: self.mode(),
                    requested_mode: Mode::Int8,
                });
            }
            let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
            let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
            Ok(VoxelBlock {
                offset: [0, 0, z],
                shape: [nx, ny, 1],
                data,
            })
        })
    }
}

/// Bzip2-compressed MRC file writer.
///
/// Because bzip2 does not support random access, the entire file is buffered
/// in memory and compressed only on [`Bzip2Writer::finalize`]. For large
/// volumes consider using [`Writer`](crate::Writer) instead.
#[derive(Debug)]
pub struct Bzip2Writer {
    header: Header,
    data: Vec<u8>,
    path: std::path::PathBuf,
    data_offset: usize,
    bytes_per_voxel: usize,
    shape: VolumeShape,
}

impl Bzip2Writer {
    /// Create a new bzip2-compressed MRC file.
    pub fn create<P: AsRef<Path>>(path: P, header: Header) -> Result<Self, Error> {
        let mut header = header;
        header.set_file_endian(FileEndian::LittleEndian);
        header.validate_detailed()?;

        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let data = vec![0u8; data_size];

        let mode = Mode::from_i32(header.mode).ok_or(Error::UnsupportedMode)?;
        if mode == Mode::Packed4Bit {
            return Err(Error::UnsupportedMode);
        }
        let bytes_per_voxel = mode.byte_size();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok(Self {
            header,
            data,
            path: path.as_ref().to_path_buf(),
            data_offset: header.data_offset(),
            bytes_per_voxel,
            shape,
        })
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

    pub fn write_block<T: Voxel>(&mut self, block: &VoxelBlock<T>) -> Result<(), Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if !self.shape.contains_block(block.offset, block.shape) {
            return Err(Error::BoundsError);
        }

        let [nx, ny, _nz] = [self.shape.nx, self.shape.ny, self.shape.nz];
        let [ox, oy, oz] = block.offset;
        let [sx, sy, sz] = block.shape;

        let linear = ox + oy * nx + oz * nx * ny;
        let start_byte = self.data_offset + linear * self.bytes_per_voxel;
        let byte_len = sx * sy * sz * self.bytes_per_voxel;

        let mut buffer = vec![0u8; byte_len];
        let file_endian = self.header.detect_endian();
        crate::engine::codec::encode_slice(&block.data, &mut buffer, file_endian);

        self.data[start_byte..start_byte + byte_len].copy_from_slice(&buffer);
        Ok(())
    }

    pub fn finalize(self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);

        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        encoder.write_all(&header_bytes)?;

        let ext_size = self.header.nsymbt as usize;
        if ext_size > 0 {
            encoder.write_all(&vec![0u8; ext_size])?;
        }

        encoder.write_all(&self.data)?;
        let compressed = encoder.finish()?;

        let mut file = File::create(&self.path)?;
        file.write_all(&compressed)?;
        Ok(())
    }
}
