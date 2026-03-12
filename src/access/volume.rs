//! Volume container for MRC data
//!
//! Generic volume container with compile-time type safety.

use crate::core::{AxisMap, Error, Mode, check_bounds};
use crate::header::Header;
use crate::voxel::{Encoding, FileEndian, Voxel, validate_mode};

#[cfg(feature = "std")]
use alloc::vec;
#[cfg(feature = "std")]
use alloc::vec::Vec;

/// A volume of voxel data with configurable dimensionality
///
/// # Type Parameters
/// - `T`: Voxel type (must implement Voxel + Encoding)
/// - `S`: Storage backend (must implement AsRef<[u8]> for read, AsMut<[u8]> for write)
/// - `D`: Dimensionality (default 3 for 3D volumes)
#[derive(Debug)]
pub struct Volume<T, S, const D: usize = 3> {
    header: Header,
    storage: S,
    shape: [usize; D],
    /// Strides for linear indexing (accounts for axis_map)
    strides: [usize; D],
    _marker: core::marker::PhantomData<T>,
}

impl<T, S, const D: usize> Volume<T, S, D> {
    /// Get the header
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the shape (logical dimensions: nx, ny, nz)
    pub fn shape(&self) -> &[usize; D] {
        &self.shape
    }

    /// Get the strides
    pub fn strides(&self) -> &[usize; D] {
        &self.strides
    }

    /// Get the axis map
    pub fn axis_map(&self) -> &AxisMap {
        &self.header.axis_map
    }

    /// Total number of voxels
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S, 1> {
    /// Create a 1D volume from raw data (replaces DataBlock)
    ///
    /// This is a simplified constructor for 1D data without full header metadata.
    /// For 3D volumes with full header information, use `Volume::new()` with a Header.
    pub fn from_raw(data: S, mode: Mode, endian: FileEndian) -> Result<Self, Error> {
        validate_mode::<T>(mode)?;

        let voxel_count = data.as_ref().len() / T::SIZE;
        let shape = [voxel_count];
        let strides = [1];

        // Create minimal header
        let mut header = Header::new();
        header.set_dimensions(voxel_count, 1, 1);
        header.set_mode(mode);
        header.file_endian = endian;

        Ok(Self {
            header,
            storage: data,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Create a 1D volume with explicit voxel count
    pub fn from_raw_with_count(data: S, mode: Mode, endian: FileEndian, voxel_count: usize) -> Result<Self, Error> {
        validate_mode::<T>(mode)?;

        let expected_size = voxel_count * T::SIZE;
        if data.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: data.as_ref().len(),
            });
        }

        let shape = [voxel_count];
        let strides = [1];

        let mut header = Header::new();
        header.set_dimensions(voxel_count, 1, 1);
        header.set_mode(mode);
        header.file_endian = endian;

        Ok(Self {
            header,
            storage: data,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Get voxel at index (1D)
    pub fn get_1d(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        T::decode(self.header.file_endian, &self.storage.as_ref()[offset..offset + T::SIZE])
    }

    /// Get voxel at index with bounds checking
    pub fn get_1d_checked(&self, index: usize) -> Option<T> {
        if index >= self.shape[0] {
            return None;
        }
        Some(self.get_1d(index))
    }
}

impl<T: Voxel + Encoding, S: AsMut<[u8]>> Volume<T, S, 1> {
    /// Set voxel at index (1D)
    pub fn set_1d(&mut self, index: usize, value: T) {
        let offset = index * T::SIZE;
        value.encode(self.header.file_endian, &mut self.storage.as_mut()[offset..offset + T::SIZE]);
    }

    /// Set voxel at index with bounds checking
    pub fn set_1d_checked(&mut self, index: usize, value: T) -> Result<(), Error> {
        check_bounds(index, self.shape[0])?;
        self.set_1d(index, value);
        Ok(())
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S, 3> {
    /// Create a new 3D volume from header and storage
    ///
    /// The strides are calculated from the axis_map in the header,
    /// which defines the storage order of the data.
    pub fn new(header: Header, storage: S) -> Result<Self, Error> {
        // Validate mode matches
        validate_mode::<T>(header.mode())?;

        let shape = [header.nx(), header.ny(), header.nz()];
        let total = shape[0]
            .checked_mul(shape[1])
            .and_then(|v| v.checked_mul(shape[2]))
            .ok_or(Error::InvalidDimensions)?;

        // Validate size
        let expected_size = total.checked_mul(T::SIZE).ok_or(Error::InvalidDimensions)?;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }

        // Calculate strides based on axis_map
        // MRC data is stored in column-major order (column varies fastest)
        // axis_map tells us which logical dimension (X, Y, Z) corresponds to each storage axis
        let strides = header.axis_map.strides(shape);

        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Create from dimensions and data (standard axis map)
    pub fn from_data(
        nx: usize,
        ny: usize,
        nz: usize,
        endian: crate::FileEndian,
        storage: S,
    ) -> Result<Self, Error> {
        let shape = [nx, ny, nz];
        let total = nx
            .checked_mul(ny)
            .and_then(|v| v.checked_mul(nz))
            .ok_or(Error::InvalidDimensions)?;

        let expected_size = total.checked_mul(T::SIZE).ok_or(Error::InvalidDimensions)?;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }

        let mut header = Header::new();
        header.set_dimensions(nx, ny, nz);
        header.set_mode(<T as Voxel>::MODE);
        header.file_endian = endian;

        // Standard strides for X=column, Y=row, Z=section
        let strides = [1, nx, nx * ny];

        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Get dimensions as tuple
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.shape[0], self.shape[1], self.shape[2])
    }

    /// Get a voxel at linear index
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn get(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        T::decode(self.header.file_endian, &bytes[offset..offset + T::SIZE])
    }

    /// Get a voxel at linear index, returning None if out of bounds
    pub fn get_checked(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        Some(T::decode(
            self.header.file_endian,
            &bytes[offset..offset + T::SIZE],
        ))
    }

    /// Get a voxel at logical 3D coordinates (x, y, z)
    ///
    /// The coordinates are in logical space (X, Y, Z). The stride calculation
    /// accounts for the axis_map to correctly access the stored data.
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get_at(&self, x: usize, y: usize, z: usize) -> T {
        let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
        self.get(index)
    }

    /// Get a voxel at 3D coordinates, returning None if out of bounds
    pub fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Option<T> {
        if x >= self.shape[0] || y >= self.shape[1] || z >= self.shape[2] {
            return None;
        }
        Some(self.get_at(x, y, z))
    }

    /// Iterate over all voxels in storage order
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian;
        let bytes = self.storage.as_ref();
        let len = self.len();
        (0..len).map(move |i| {
            let offset = i * T::SIZE;
            T::decode(endian, &bytes[offset..offset + T::SIZE])
        })
    }

    /// Iterate over voxels in logical order (X varies fastest)
    pub fn iter_logical(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian;
        let bytes = self.storage.as_ref();
        let (nx, ny, nz) = self.dimensions();
        let strides = self.strides;

        (0..nz).flat_map(move |z| {
            (0..ny).flat_map(move |y| {
                (0..nx).map(move |x| {
                    let index = x * strides[0] + y * strides[1] + z * strides[2];
                    let offset = index * T::SIZE;
                    T::decode(endian, &bytes[offset..offset + T::SIZE])
                })
            })
        })
    }

    /// Get slice of raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.storage.as_ref()
    }

    /// Zero-copy slice view (native endianness only)
    ///
    /// Returns a typed slice view of the voxel data without copying or decoding.
    /// This only works when:
    /// 1. The file endianness matches the native system endianness
    /// 2. The data is properly aligned for type T
    /// 3. The storage is contiguous (standard axis map)
    ///
    /// # Errors
    /// - `Error::WrongEndianness` if file endianness doesn't match native
    /// - `Error::MisalignedData` if data is not properly aligned
    /// - `Error::InvalidAxisMap` if axis mapping is non-standard
    ///
    /// # Example
    /// ```no_run
    /// # fn example(volume: &mrc::Volume<f32, Vec<u8>>) -> Result<(), mrc::Error> {
    /// if let Ok(slice) = volume.as_slice() {
    ///     // Zero-copy access - slice[i] is valid
    ///     let sum: f32 = slice.iter().sum();
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn as_slice(&self) -> Result<&[T], Error>
    where
        T: bytemuck::Pod,
    {
        // Check native endianness
        if !self.header.file_endian.is_native() {
            return Err(Error::WrongEndianness);
        }

        // Check for standard axis map (contiguous storage)
        if !self.header.axis_map.is_standard() {
            return Err(Error::InvalidAxisMap);
        }

        // Try to cast the byte slice
        let bytes = self.storage.as_ref();
        let expected_len = self.len();
        let expected_bytes = expected_len * T::SIZE;

        bytemuck::try_cast_slice(&bytes[..expected_bytes]).map_err(|_| Error::MisalignedData {
            required: core::mem::align_of::<T>(),
            actual: bytes.as_ptr().align_offset(core::mem::align_of::<T>()),
        })
    }

    /// Try zero-copy slice, fall back to decoded Vec on failure
    ///
    /// Returns a `Cow<[T]>` that borrows the data when possible (native endianness,
    /// proper alignment, standard axis map) or allocates a decoded copy otherwise.
    #[cfg(feature = "std")]
    pub fn to_slice_cow(&self) -> alloc::borrow::Cow<'_, [T]>
    where
        T: bytemuck::Pod + Clone,
    {
        if let Ok(slice) = self.as_slice() {
            alloc::borrow::Cow::Borrowed(slice)
        } else {
            alloc::borrow::Cow::Owned(self.iter().collect())
        }
    }

    /// Copy voxels to a pre-allocated buffer
    ///
    /// For native endianness with proper alignment, uses fast memcpy.
    /// Otherwise, decodes element-by-element.
    pub fn copy_to(&self, out: &mut [T]) -> Result<(), Error>
    where
        T: bytemuck::Pod,
    {
        let n = self.len();
        if out.len() < n {
            return Err(Error::BufferTooSmall {
                expected: n,
                got: out.len(),
            });
        }

        // Fast path: native endianness, standard axis map, proper alignment
        if self.header.file_endian.is_native() && self.header.axis_map.is_standard() {
            if let Ok(slice) = self.as_slice() {
                out[..n].copy_from_slice(&slice[..n]);
                return Ok(());
            }
        }

        // Fallback: decode element-by-element
        let bytes = self.storage.as_ref();
        for (i, dst) in out[..n].iter_mut().enumerate() {
            let offset = i * T::SIZE;
            *dst = T::decode(self.header.file_endian, &bytes[offset..offset + T::SIZE]);
        }

        Ok(())
    }

    /// Convert to owned Vec
    ///
    /// For native endianness with proper alignment and standard axis map,
    /// uses fast slice copy. Otherwise decodes element-by-element.
    #[cfg(feature = "std")]
    pub fn to_vec(&self) -> alloc::vec::Vec<T>
    where
        T: bytemuck::Pod + Default,
    {
        // Fast path
        if let Ok(slice) = self.as_slice() {
            return slice.to_vec();
        }

        // Fallback
        self.iter().collect()
    }

    /// Convert linear index to logical coordinates (x, y, z)
    ///
    /// This assumes standard C-order indexing and may not be correct
    /// for non-standard axis_map values.
    pub fn coords_of(&self, index: usize) -> (usize, usize, usize) {
        let (nx, ny, _) = self.dimensions();
        let z = index / (nx * ny);
        let remainder = index % (nx * ny);
        let y = remainder / nx;
        let x = remainder % nx;
        (x, y, z)
    }

    /// Convert logical coordinates to linear index
    pub fn index_of(&self, x: usize, y: usize, z: usize) -> usize {
        x * self.strides[0] + y * self.strides[1] + z * self.strides[2]
    }

    /// Compute statistics (min, max, mean, rms) for the volume data
    ///
    /// Returns a `Statistics` struct with values calculated from the actual data.
    /// This is useful for updating header statistics after modifying voxel values.
    ///
    /// # Type Requirements
    /// This method is only available for types that can be converted to f64.
    pub fn compute_statistics(&self) -> Statistics
    where
        T: Into<f64>,
    {
        crate::stats::compute_stats(self.iter())
    }

    /// Extract a 2D slice from the volume at a specific Z position
    ///
    /// Returns an `Image2D` view into the original volume data.
    ///
    /// # Arguments
    /// * `z` - The Z index of the slice to extract
    ///
    /// # Errors
    /// Returns `Error::IndexOutOfBounds` if z is out of range
    pub fn slice(&self, z: usize) -> Result<Image2D<T, &[u8]>, Error> {
        check_bounds(z, self.shape[2])?;

        let nx = self.shape[0];
        let ny = self.shape[1];
        let slice_size = nx * ny * T::SIZE;
        let slice_offset = z * slice_size;

        let bytes = self.storage.as_ref();
        let slice_data = &bytes[slice_offset..slice_offset + slice_size];

        Image2D::new_2d(nx, ny, self.header.file_endian, slice_data)
    }

    /// Extract a subvolume with the specified bounds
    ///
    /// Returns a new Volume with copied data for the specified region.
    ///
    /// # Arguments
    /// * `x_start` - Starting X index (inclusive)
    /// * `x_end` - Ending X index (exclusive)
    /// * `y_start` - Starting Y index (inclusive)
    /// * `y_end` - Ending Y index (exclusive)
    /// * `z_start` - Starting Z index (inclusive)
    /// * `z_end` - Ending Z index (exclusive)
    ///
    /// # Errors
    /// Returns `Error::IndexOutOfBounds` if any index is out of range
    #[cfg(feature = "std")]
    pub fn subvolume(
        &self,
        x_start: usize,
        x_end: usize,
        y_start: usize,
        y_end: usize,
        z_start: usize,
        z_end: usize,
    ) -> Result<Volume<T, Vec<u8>, 3>, Error>
    where
        T: Default + Clone,
    {
        // Validate bounds
        if x_start >= self.shape[0] || x_end > self.shape[0] || x_start >= x_end {
            return Err(Error::IndexOutOfBounds {
                index: x_start,
                length: self.shape[0],
            });
        }
        if y_start >= self.shape[1] || y_end > self.shape[1] || y_start >= y_end {
            return Err(Error::IndexOutOfBounds {
                index: y_start,
                length: self.shape[1],
            });
        }
        if z_start >= self.shape[2] || z_end > self.shape[2] || z_start >= z_end {
            return Err(Error::IndexOutOfBounds {
                index: z_start,
                length: self.shape[2],
            });
        }

        let new_nx = x_end - x_start;
        let new_ny = y_end - y_start;
        let new_nz = z_end - z_start;
        let voxel_count = new_nx * new_ny * new_nz;

        // Allocate buffer for subvolume
        let mut new_data = vec![0u8; voxel_count * T::SIZE];

        // Copy data from source region
        for (new_z, src_z) in (z_start..z_end).enumerate() {
            for (new_y, src_y) in (y_start..y_end).enumerate() {
                for (new_x, src_x) in (x_start..x_end).enumerate() {
                    let src_idx = self.index_of(src_x, src_y, src_z);
                    let dst_idx = new_z * new_ny * new_nx + new_y * new_nx + new_x;

                    let src_offset = src_idx * T::SIZE;
                    let dst_offset = dst_idx * T::SIZE;

                    new_data[dst_offset..dst_offset + T::SIZE]
                        .copy_from_slice(&self.storage.as_ref()[src_offset..src_offset + T::SIZE]);
                }
            }
        }

        // Create header for subvolume
        let mut new_header = self.header.clone();
        new_header.set_dimensions(new_nx, new_ny, new_nz);
        // Update origin to reflect subvolume position
        let (dx, dy, dz) = new_header.voxel_size();
        let new_origin = [
            new_header.xorigin() + x_start as f32 * dx,
            new_header.yorigin() + y_start as f32 * dy,
            new_header.zorigin() + z_start as f32 * dz,
        ];
        new_header.set_origin(new_origin[0], new_origin[1], new_origin[2]);

        Volume::new(new_header, new_data)
    }
}

// Re-export Statistics from stats module
pub use crate::stats::Statistics;

impl<T: Voxel + Encoding, S: AsMut<[u8]>> Volume<T, S, 3> {
    /// Get mutable access to raw bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.storage.as_mut()
    }

    /// Set a voxel at linear index
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn set(&mut self, index: usize, value: T) {
        let offset = index * T::SIZE;
        let bytes = self.storage.as_mut();
        value.encode(
            self.header.file_endian,
            &mut bytes[offset..offset + T::SIZE],
        );
    }

    /// Set a voxel at linear index, returning error if out of bounds
    pub fn set_checked(&mut self, index: usize, value: T) -> Result<(), Error> {
        check_bounds(index, self.len())?;
        self.set(index, value);
        Ok(())
    }

    /// Set a voxel at logical 3D coordinates (x, y, z)
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn set_at(&mut self, x: usize, y: usize, z: usize, value: T) {
        let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
        self.set(index, value);
    }

    /// Set a voxel at 3D coordinates, returning error if out of bounds
    pub fn set_at_checked(&mut self, x: usize, y: usize, z: usize, value: T) -> Result<(), Error> {
        if x >= self.shape[0] || y >= self.shape[1] || z >= self.shape[2] {
            return Err(Error::IndexOutOfBounds {
                index: x + y * self.shape[0] + z * self.shape[0] * self.shape[1],
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }
}

// Strides calculation now uses AxisMap::strides() method

/// 2D volume (image slice)
pub type Image2D<T, S> = Volume<T, S, 2>;

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S, 2> {
    /// Create a new 2D image from storage
    pub fn new_2d(
        nx: usize,
        ny: usize,
        endian: crate::FileEndian,
        storage: S,
    ) -> Result<Self, Error> {
        let shape = [nx, ny];
        let total = nx.checked_mul(ny).ok_or(Error::InvalidDimensions)?;

        let expected_size = total.checked_mul(T::SIZE).ok_or(Error::InvalidDimensions)?;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }

        let mut header = Header::new();
        header.set_dimensions(nx, ny, 1);
        header.set_mode(<T as Voxel>::MODE);
        header.file_endian = endian;

        let strides = [1, nx];

        Ok(Self {
            header,
            storage,
            shape,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Get a pixel at 2D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get_pixel(&self, x: usize, y: usize) -> T {
        let index = y * self.strides[1] + x * self.strides[0];
        let offset = index * T::SIZE;
        T::decode(
            self.header.file_endian,
            &self.storage.as_ref()[offset..offset + T::SIZE],
        )
    }

    /// Get a pixel at 2D coordinates, returning None if out of bounds
    pub fn get_pixel_checked(&self, x: usize, y: usize) -> Option<T> {
        if x >= self.shape[0] || y >= self.shape[1] {
            return None;
        }
        Some(self.get_pixel(x, y))
    }
}

// ============================================================================
// Implement unified access traits
// ============================================================================

use super::{VoxelAccess, VoxelAccessMut};

impl<T: Voxel + Encoding, S: AsRef<[u8]>, const D: usize> VoxelAccess for Volume<T, S, D> {
    fn mode(&self) -> crate::core::Mode {
        self.header.mode()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get<V: Voxel + Encoding>(&self, index: usize) -> Result<V, Error> {
        validate_mode::<V>(self.header.mode())?;
        check_bounds(index, self.len())?;
        let offset = index * V::SIZE;
        let bytes = self.storage.as_ref();
        Ok(V::decode(
            self.header.file_endian,
            &bytes[offset..offset + V::SIZE],
        ))
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]> + AsMut<[u8]>, const D: usize> VoxelAccessMut for Volume<T, S, D> {
    fn set<V: Voxel + Encoding>(&mut self, index: usize, value: V) -> Result<(), Error> {
        validate_mode::<V>(self.header.mode())?;
        check_bounds(index, self.len())?;

        // Special handling for Packed4Bit (two values per byte)
        if <V as Voxel>::MODE == crate::core::Mode::Packed4Bit {
            let byte_index = index / 2;
            let is_second = index % 2 == 1;
            let bytes = self.storage.as_mut();

            // Get the new nibble value by encoding to a temporary buffer
            let mut temp = [0u8; 1];
            value.encode(self.header.file_endian, &mut temp);
            let new_nibble = temp[0] & 0x0F;

            if is_second {
                bytes[byte_index] = (bytes[byte_index] & 0x0F) | (new_nibble << 4);
            } else {
                bytes[byte_index] = (bytes[byte_index] & 0xF0) | new_nibble;
            }
            Ok(())
        } else {
            let offset = index * V::SIZE;
            let bytes = self.storage.as_mut();
            value.encode(
                self.header.file_endian,
                &mut bytes[offset..offset + V::SIZE],
            );
            Ok(())
        }
    }
}

// Type aliases for common volume types

/// Volume with Vec<u8> storage (most common)
pub type VecVolume<T, const D: usize = 3> = Volume<T, Vec<u8>, D>;

/// Volume with memory-mapped storage (read-only)
#[cfg(feature = "mmap")]
pub type MmapVolume<T, const D: usize = 3> = Volume<T, memmap2::Mmap, D>;

/// Volume with mutable memory-mapped storage
#[cfg(feature = "mmap")]
pub type MmapVolumeMut<T, const D: usize = 3> = Volume<T, memmap2::MmapMut, D>;

// Implement Volume trait for Volume<T, S, 3>
use super::volume_trait::{Volume as VolumeTrait, VolumeStats};

impl<T: Voxel + Encoding, S: AsRef<[u8]>> VolumeTrait for Volume<T, S, 3> {
    type Voxel = T;

    fn header(&self) -> &Header {
        &self.header
    }
    fn shape(&self) -> (usize, usize, usize) {
        (self.shape[0], self.shape[1], self.shape[2])
    }

    fn strides(&self) -> (usize, usize, usize) {
        (self.strides[0], self.strides[1], self.strides[2])
    }

    unsafe fn get_unchecked(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        T::decode(self.header.file_endian, &bytes[offset..offset + T::SIZE])
    }

    fn compute_stats(&self) -> VolumeStats
    where
        T: Into<f64>,
    {
        crate::stats::compute_stats((0..self.len()).map(|i| unsafe { self.get_unchecked(i) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_axis_map_strides() {
        // Standard: X=column, Y=row, Z=section
        let axis_map = AxisMap::new(1, 2, 3);
        let shape = [64, 64, 64];
        let strides = axis_map.strides(shape);

        // X should have stride 1 (column, fastest)
        // Y should have stride 64 (row)
        // Z should have stride 4096 (section, slowest)
        assert_eq!(strides, [1, 64, 4096]);
    }

    #[test]
    fn test_nonstandard_axis_map_strides() {
        // Non-standard: Z=column, Y=row, X=section
        let axis_map = AxisMap::new(3, 2, 1);
        let shape = [64, 64, 64];
        let strides = axis_map.strides(shape);

        // X (stored as section) should have stride 4096
        // Y (stored as row) should have stride 64
        // Z (stored as column) should have stride 1
        assert_eq!(strides, [4096, 64, 1]);
    }
}
