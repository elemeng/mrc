//! MRC file reader with iterator-centric API

use crate::engine::block::VolumeShape;
use crate::engine::codec::{EndianCodec, decode_slice};
use crate::engine::endian::FileEndian;
use crate::engine::pipeline::is_zero_copy;
use crate::iter::{BlockIter, SlabIter, SliceIter};
use crate::{Error, Header, Mode};

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
            let shape =
                VolumeShape::new(header.nx as usize, header.ny as usize, header.nz as usize);

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

    pub fn slices<T>(&self) -> SliceIter<'_, T> {
        SliceIter::new(self, self.shape)
    }

    pub fn slabs<T>(&self, k: usize) -> SlabIter<'_, T> {
        SlabIter::new(self, self.shape, k)
    }

    pub fn blocks<T>(&self, chunk_shape: [usize; 3]) -> BlockIter<'_, T> {
        BlockIter::new(self, self.shape, chunk_shape)
    }

    pub(crate) fn read_voxels(
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

        let bytes_per_voxel = self.mode().byte_size();

        let start_byte = (ox + oy * nx + oz * nx * ny) * bytes_per_voxel;
        let end_byte = start_byte + sx * sy * sz * bytes_per_voxel;

        if end_byte > self.data.len() {
            return Err(Error::BoundsError);
        }

        Ok(self.data[start_byte..end_byte].to_vec())
    }

    /// Decode a block of voxels to the specified type.
    ///
    /// This is the unified decode pipeline that handles:
    /// - Layer 1 → Layer 2: Raw bytes → Endian normalization
    /// - Layer 2 → Layer 3: Endian-normalized → Typed values
    ///
    /// Uses zero-copy fast path when:
    /// - src_mode == dst_mode (T matches file mode)
    /// - file_endian == native
    pub(crate) fn decode_block<T: EndianCodec + Send + Copy + Default>(
        &self,
        bytes: &[u8],
    ) -> Result<Vec<T>, Error> {
        // Standard decode path with safe initialization
        Ok(decode_slice(bytes, self.endian))
    }

    /// Check if zero-copy decode is possible for the given type.
    /// Zero-copy requires: file mode matches T's mode AND file endian is native.
    pub fn can_zero_copy<T: crate::mode::Voxel>(&self) -> bool {
        is_zero_copy(self.mode(), T::MODE, self.endian)
    }
}

impl Reader {
    /// Get a reference to the raw data bytes
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the file endianness
    pub fn endian(&self) -> FileEndian {
        self.endian
    }
}
