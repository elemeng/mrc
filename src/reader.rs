//! MRC file reader with iterator-centric API

use crate::engine::block::VolumeShape;
use crate::engine::codec::{EndianCodec, decode_slice};
use crate::engine::endian::FileEndian;
use crate::iter::{BlockIter, SlabIter, SliceIter};
use crate::mode::Voxel;
use crate::{Error, Header, Mode};

use std::vec::Vec;

/// Iterator over slices yielding `f32` voxel blocks.
pub type SliceIterF32<'a> =
    Box<dyn Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + 'a>;

#[derive(Debug)]
pub struct Reader {
    header: Header,
    ext_header: Vec<u8>,
    data: Vec<u8>,
    endian: FileEndian,
    shape: VolumeShape,
}

impl Reader {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        Self::_open(path, false).map(|(r, _)| r)
    }

    /// Open an MRC file in **permissive** mode.
    ///
    /// Non-fatal header issues (unusual MAP field, unexpected `nversion`,
    /// non-standard axis mapping, etc.) are collected as warning strings
    /// instead of causing a hard error. Only genuinely unreadable files
    /// (negative dimensions, unsupported mode, IO failure) return `Err`.
    pub fn open_permissive<P: AsRef<std::path::Path>>(path: P) -> Result<(Self, Vec<String>), Error> {
        Self::_open(path, true)
    }

    fn _open<P: AsRef<std::path::Path>>(path: P, permissive: bool) -> Result<(Self, Vec<String>), Error> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;

        let mut header_bytes = [0u8; 1024];
        file.read_exact(&mut header_bytes)?;

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
        let mut ext_header = vec![0u8; ext_size];
        if ext_size > 0 {
            file.read_exact(&mut ext_header)?;
        }

        let mut data = vec![0u8; data_size];
        file.read_exact(&mut data)?;

        if !permissive {
            // Check for trailing bytes (file larger than header + ext_header + data)
            let mut trailing = [0u8; 1];
            if file.read(&mut trailing)? > 0 {
                return Err(Error::FileSizeMismatch {
                    expected: header.data_offset() + data_size,
                    actual: header.data_offset() + data_size + 1, // at least 1 extra byte
                });
            }
        }

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

    pub fn slices<T: Voxel>(&self) -> SliceIter<'_, T> {
        SliceIter::new(self, self.shape)
    }

    pub fn slabs<T: Voxel>(&self, k: usize) -> SlabIter<'_, T> {
        SlabIter::new(self, self.shape, k)
    }

    pub fn blocks<T: Voxel>(&self, chunk_shape: [usize; 3]) -> BlockIter<'_, T> {
        BlockIter::new(self, self.shape, chunk_shape)
    }

    /// Read a block of raw voxel bytes from the file.
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

    /// Read and decode a block of voxels to the specified type.
    ///
    /// Returns an error if `T` does not match the file's voxel mode.
    pub fn read_block<T: Voxel>(&self, offset: [usize; 3], shape: [usize; 3]) -> Result<crate::engine::block::VoxelBlock<T>, Error> {
        let bytes = self.read_block_bytes(offset, shape)?;
        let data = self.decode_block::<T>(&bytes)?;
        Ok(crate::engine::block::VoxelBlock {
            offset,
            shape,
            data,
        })
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// # Errors
    /// Returns `Error::ModeMismatch` if `T` does not match the file mode.
    pub(crate) fn decode_block<T: Voxel>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
        if T::MODE != self.mode() {
            return Err(Error::ModeMismatch {
                file_mode: self.mode(),
                requested_mode: T::MODE,
            });
        }

        if self.endian == FileEndian::native() {
            self.decode_block_native_endian(bytes)
        } else {
            Ok(decode_slice(bytes, self.endian))
        }
    }

    /// Native-endian decode: memcpy bytes directly to Vec<T>.
    fn decode_block_native_endian<T: EndianCodec + Copy>(&self, bytes: &[u8]) -> Result<Vec<T>, Error> {
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
    }

    /// Iterate over slices, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slices_f32(&self) -> Result<SliceIterF32<'_>, Error> {
        use crate::engine::block::VoxelBlock;

        match self.mode() {
            Mode::Float32 => Ok(Box::new(self.slices::<f32>())),
            Mode::Int16 => Ok(Box::new(self.slices::<i16>().map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Uint16 => Ok(Box::new(self.slices::<u16>().map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_u16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Int8 => Ok(Box::new(self.slices::<i8>().map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i8_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            _ => Err(Error::UnsupportedMode),
        }
    }

    /// Iterate over slabs, automatically converting common types to `f32`.
    ///
    /// Supported source modes: `Float32`, `Int16`, `Uint16`, `Int8`.
    pub fn slabs_f32(&self, k: usize) -> Result<SliceIterF32<'_>, Error> {
        use crate::engine::block::VoxelBlock;

        match self.mode() {
            Mode::Float32 => Ok(Box::new(self.slabs::<f32>(k))),
            Mode::Int16 => Ok(Box::new(self.slabs::<i16>(k).map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Uint16 => Ok(Box::new(self.slabs::<u16>(k).map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_u16_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            Mode::Int8 => Ok(Box::new(self.slabs::<i8>(k).map(|r| {
                let b = r?;
                let data = crate::engine::convert::convert_i8_slice_to_f32(&b.data);
                Ok(VoxelBlock {
                    offset: b.offset,
                    shape: b.shape,
                    data,
                })
            }))),
            _ => Err(Error::UnsupportedMode),
        }
    }

    /// Iterate over slices for Mode 0 (8-bit) files with signed/unsigned interpretation.
    ///
    /// Mode 0 files are ambiguous: some software writes signed bytes, others unsigned.
    /// This method lets you explicitly choose the interpretation and returns `f32` values.
    pub fn slices_mode0(
        &self,
        interp: crate::mode::M0Interpretation,
    ) -> impl Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + '_ {
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
            Ok(crate::engine::block::VoxelBlock {
                offset: [0, 0, z],
                shape: [nx, ny, 1],
                data,
            })
        })
    }

    /// Iterate over slices, automatically converting Mode 6 (`Uint16`) to `u8`.
    ///
    /// Returns an error if the file is not Mode 6 or if any value exceeds 255.
    pub fn slices_u8(&self) -> Result<impl Iterator<Item = Result<crate::engine::block::VoxelBlock<u8>, Error>> + '_, Error> {
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
            Ok(crate::engine::block::VoxelBlock {
                offset: [0, 0, z],
                shape: [nx, ny, 1],
                data: u8_data,
            })
        }))
    }

    /// Iterate over slabs for Mode 0 (8-bit) files with signed/unsigned interpretation.
    pub fn slabs_mode0(
        &self,
        k: usize,
        interp: crate::mode::M0Interpretation,
    ) -> impl Iterator<Item = Result<crate::engine::block::VoxelBlock<f32>, Error>> + '_ {
        let nx = self.shape.nx;
        let ny = self.shape.ny;
        let nz = self.shape.nz;
        let mut z = 0usize;
        let mut error_returned = false;
        std::iter::from_fn(move || {
            if error_returned {
                return None;
            }
            if self.mode() != Mode::Int8 {
                error_returned = true;
                return Some(Err(Error::ModeMismatch {
                    file_mode: self.mode(),
                    requested_mode: Mode::Int8,
                }));
            }
            if z >= nz {
                return None;
            }
            let start = z;
            let size = k.min(nz - z);
            z += size;
            let bytes = match self.read_block_bytes([0, 0, start], [nx, ny, size]) {
                Ok(b) => b,
                Err(e) => return Some(Err(e)),
            };
            let data = crate::engine::convert::reinterpret_m0(&bytes, interp);
            Some(Ok(crate::engine::block::VoxelBlock {
                offset: [0, 0, start],
                shape: [nx, ny, size],
                data,
            }))
        })
    }
}

impl Reader {
    /// Get a reference to the raw data bytes
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Cross-check header statistics against actual data.
    ///
    /// Computes `dmin`, `dmax`, `dmean` and `rms` from the data block and
    /// compares them with the header values using a 1 % relative tolerance
    /// (matching Python `mrcfile`'s `np.isclose(rtol=0.01)`).
    ///
    /// # Errors
    /// Returns [`Error::StatsMismatch`] if any statistic deviates by more than 1 %.
    pub fn validate_header_stats(&self) -> Result<(), Error> {
        crate::engine::stats::validate_header_stats(&self.header, &self.data)
    }

    /// Get the file endianness
    pub fn endian(&self) -> FileEndian {
        self.endian
    }

    /// Get the extended header bytes, if any.
    pub fn ext_header_bytes(&self) -> &[u8] {
        &self.ext_header
    }
}
