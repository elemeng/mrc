//! Gzip-compressed MRC file reader and writer.
//!
//! Because gzip does not support random access, the writer buffers the entire
//! file in memory and compresses on [`GzipWriter::finalize`]. This matches the
//! behaviour of the reference Python `mrcfile` library.

use crate::engine::block::{VolumeShape, VoxelBlock};
use crate::engine::endian::FileEndian;
use crate::iter::{BlockIter, SliceIter, SlabIter};
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Gzip-compressed MRC file reader.
///
/// The entire file is decompressed into memory on open, after which the API
/// is identical to [`Reader`](crate::Reader).
#[derive(Debug)]
pub struct GzipReader {
    header: Header,
    ext_header: Vec<u8>,
    data: Vec<u8>,
    endian: FileEndian,
    shape: VolumeShape,
}

impl GzipReader {
    /// Open a gzip-compressed MRC file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::_open(path, false).map(|(r, _)| r)
    }

    /// Open a gzip-compressed MRC file in **permissive** mode.
    ///
    /// Non-fatal header issues are collected as warning strings instead of
    /// causing hard errors.
    pub fn open_permissive<P: AsRef<Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        Self::_open(path, true)
    }

    fn _open<P: AsRef<Path>>(path: P, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        let file = File::open(path)?;
        let mut decoder = flate2::read::GzDecoder::new(file);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;

        if buf.len() < 1024 {
            return Err(Error::InvalidHeader);
        }

        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&buf[..1024]);
        let (header, endian_warning) = Header::decode_from_bytes_with_info(&header_bytes);

        let mut warnings = if permissive {
            header.validate_permissive().map_err(Error::InvalidHeaderDetailed)?
        } else {
            header.validate_detailed().map_err(Error::InvalidHeaderDetailed)?;
            Vec::new()
        };

        if let Some(msg) = endian_warning {
            warnings.push(msg.to_string());
        }

        let data_size = header.data_size().ok_or(Error::InvalidHeader)?;
        let ext_size = header.nsymbt as usize;

        if !permissive {
            if buf.len() != 1024 + ext_size + data_size {
                return Err(Error::FileSizeMismatch {
                    expected: 1024 + ext_size + data_size,
                    actual: buf.len(),
                });
            }
        } else if buf.len() != 1024 + ext_size + data_size {
            warnings.push(format!(
                "File size mismatch: expected {} bytes, got {}",
                1024 + ext_size + data_size,
                buf.len()
            ));
        }

        let ext_header = buf[1024..1024 + ext_size].to_vec();
        let data = buf[1024 + ext_size..].to_vec();

        let endian = header.detect_endian();
        let shape = VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

        Ok((Self {
            header,
            ext_header,
            data,
            endian,
            shape,
        }, warnings))
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

    pub fn endian(&self) -> FileEndian {
        self.endian
    }

    pub fn ext_header_bytes(&self) -> &[u8] {
        &self.ext_header
    }

    pub fn slices<T: Voxel>(&self) -> SliceIter<'_, T, Self> {
        SliceIter::new(self, self.shape)
    }

    pub fn slabs<T: Voxel>(&self, k: usize) -> SlabIter<'_, T, Self> {
        SlabIter::new(self, self.shape, k)
    }

    pub fn blocks<T: Voxel>(&self, chunk_shape: [usize; 3]) -> BlockIter<'_, T, Self> {
        BlockIter::new(self, self.shape, chunk_shape)
    }

    pub fn read_block_bytes(
        &self,
        offset: [usize; 3],
        shape: [usize; 3],
    ) -> Result<Vec<u8>, Error> {
        let (start, end) = crate::reader_common::validate_block_read(
            self.shape, self.mode(), self.data.len(), offset, shape,
        )?;
        Ok(self.data[start..end].to_vec())
    }

    pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(VoxelBlock { offset, shape, data })
    }

    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        crate::reader_common::decode_block(bytes, self.mode(), self.endian)
    }

    pub fn slices_f32(&self) -> Result<crate::SliceIterF32<'_>, Error> {
        crate::reader_common::slices_f32(
            self.shape,
            self.mode(),
            self.endian,
            |offset, shape| self.read_block_bytes(offset, shape),
        )
    }

    /// Iterate over slabs, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slabs_f32(&self, k: usize) -> Result<crate::SliceIterF32<'_>, Error> {
        crate::reader_common::slabs_f32(
            self.shape,
            self.mode(),
            self.endian,
            k,
            |offset, shape| self.read_block_bytes(offset, shape),
        )
    }

    /// Iterate over slices, automatically converting Mode 6 (`Uint16`) to `u8`.
    ///
    /// Returns an error if the file is not Mode 6 or if any value exceeds 255.
    pub fn slices_u8(&self) -> Result<impl Iterator<Item = Result<VoxelBlock<u8>, Error>> + '_, Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        Ok((0..nz).map(move |z| {
            let bytes = self.read_block_bytes([0, 0, z], [nx, ny, 1])?;
            let u16_data = self.decode_block::<u16>(&bytes)?;
            let u8_data = crate::engine::convert::convert_u16_slice_to_u8(&u16_data)?;
            Ok(VoxelBlock {
                offset: [0, 0, z],
                shape: [nx, ny, 1],
                data: u8_data,
            })
        }))
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

    /// Cross-check header statistics against actual data.
    ///
    /// Computes `dmin`, `dmax`, `dmean` and `rms` from the decompressed data
    /// block and compares them with the header values using a 1 % relative
    /// tolerance (matching Python `mrcfile`'s `np.isclose(rtol=0.01)`).
    ///
    /// # Errors
    /// Returns [`Error::StatsMismatch`] if any statistic deviates by more than 1 %.
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        crate::engine::stats::validate_header_stats(&self.header, &self.data)
    }

    /// Get a reference to the raw decompressed data bytes.
    pub fn data_bytes(&self) -> &[u8] {
        &self.data
    }
}

/// Gzip-compressed MRC file writer.
///
/// Because gzip does not support random access, the entire file is buffered
/// in memory and compressed only on [`GzipWriter::finalize`]. For large
/// volumes consider using [`Writer`](crate::Writer) instead.
#[derive(Debug)]
pub struct GzipWriter {
    header: Header,
    data: Vec<u8>,
    path: std::path::PathBuf,
    data_offset: usize,
    bytes_per_voxel: usize,
    shape: VolumeShape,
}

impl GzipWriter {
    /// Create a new gzip-compressed MRC file.
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

    /// Write a block of `u8` data by automatically widening to `u16` (Mode 6).
    ///
    /// The file must have been created with [`Mode::Uint16`]. Each `u8` voxel
    /// is widened to `u16` before writing, matching Python `mrcfile`'s
    /// auto-conversion behaviour for `np.uint8` data.
    pub fn write_u8_block(&mut self, block: &VoxelBlock<u8>) -> Result<(), Error> {
        if self.mode() != Mode::Uint16 {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: Mode::Uint16,
            });
        }
        let widened = crate::engine::convert::convert_u8_slice_to_u16(&block.data);
        self.write_block(&VoxelBlock {
            offset: block.offset,
            shape: block.shape,
            data: widened,
        })
    }

    pub fn finalize(self) -> Result<(), Error> {
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
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
