//! Volume container for MRC data
//!
//! Generic 3D volume container with compile-time type safety.
//! For 2D access, use `Slice2D` views extracted from volumes.

use crate::core::{AxisMap, Error, check_bounds};
use crate::header::Header;
use crate::voxel::{Encoding, FileEndian, Voxel, validate_mode};

#[cfg(feature = "std")]
use alloc::vec;
#[cfg(feature = "std")]
use alloc::vec::Vec;

/// A 3D volume of voxel data
///
/// # Type Parameters
/// - `T`: Voxel type (must implement Voxel + Encoding)
/// - `S`: Storage backend (must implement AsRef<[u8]> for read, AsMut<[u8]> for write)
#[derive(Debug)]
pub struct Volume<T, S> {
    header: Header,
    storage: S,
    dimensions: [usize; 3],
    strides: [usize; 3],
    _marker: core::marker::PhantomData<T>,
}

impl<T, S> Volume<T, S> {
    /// Get the header
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the strides
    pub fn strides(&self) -> [usize; 3] {
        self.strides
    }

    /// Get the axis map
    pub fn axis_map(&self) -> &AxisMap {
        self.header.axis_map()
    }

    /// Total number of voxels
    pub fn len(&self) -> usize {
        self.dimensions[0] * self.dimensions[1] * self.dimensions[2]
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]>> Volume<T, S> {
    /// Validate dimensions and compute total voxel count
    fn validate_dimensions(dimensions: [usize; 3]) -> Result<usize, Error> {
        dimensions
            .iter()
            .try_fold(1usize, |acc, &d| acc.checked_mul(d))
            .ok_or(Error::InvalidDimensions)
    }

    /// Validate storage size against expected voxel count
    fn validate_storage_size(storage: &S, voxel_count: usize) -> Result<(), Error> {
        let expected_size = voxel_count
            .checked_mul(T::SIZE)
            .ok_or(Error::InvalidDimensions)?;
        if storage.as_ref().len() < expected_size {
            return Err(Error::BufferTooSmall {
                expected: expected_size,
                got: storage.as_ref().len(),
            });
        }
        Ok(())
    }

    /// Create a new 3D volume from header and storage
    pub fn new(header: Header, storage: S) -> Result<Self, Error> {
        validate_mode::<T>(header.mode())?;

        let dimensions = [header.nx(), header.ny(), header.nz()];
        let total = Self::validate_dimensions(dimensions)?;
        Self::validate_storage_size(&storage, total)?;

        let strides = header.axis_map().strides(dimensions);

        Ok(Self {
            header,
            storage,
            dimensions,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Create from dimensions and data
    pub fn from_data(nx: usize, ny: usize, nz: usize, storage: S) -> Result<Self, Error> {
        let dimensions = [nx, ny, nz];
        let total = Self::validate_dimensions(dimensions)?;
        Self::validate_storage_size(&storage, total)?;

        let header = Header::builder()
            .dimensions(nx, ny, nz)
            .mode(T::MODE)
            .build();

        let strides = [1, nx, nx * ny];

        Ok(Self {
            header,
            storage,
            dimensions,
            strides,
            _marker: core::marker::PhantomData,
        })
    }

    /// Get dimensions as tuple
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.dimensions[0], self.dimensions[1], self.dimensions[2])
    }

    /// Get a voxel at 3D coordinates (x, y, z)
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get_at(&self, x: usize, y: usize, z: usize) -> T {
        let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        T::decode(self.header.file_endian(), &bytes[offset..offset + T::SIZE])
    }

    /// Get a voxel at 3D coordinates, returning None if out of bounds
    pub fn get_at_opt(&self, x: usize, y: usize, z: usize) -> Option<T> {
        if x >= self.dimensions[0] || y >= self.dimensions[1] || z >= self.dimensions[2] {
            return None;
        }
        Some(self.get_at(x, y, z))
    }

    /// Get a voxel at 3D coordinates with full error context
    pub fn get_at_checked(&self, x: usize, y: usize, z: usize) -> Result<T, Error> {
        if x >= self.dimensions[0] || y >= self.dimensions[1] || z >= self.dimensions[2] {
            return Err(Error::IndexOutOfBounds {
                index: self.linear_index(x, y, z),
                length: self.len(),
            });
        }
        Ok(self.get_at(x, y, z))
    }

    /// Iterate over all voxels in storage order
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian();
        let bytes = self.storage.as_ref();
        let len = self.len();
        (0..len).map(move |i| {
            let offset = i * T::SIZE;
            T::decode(endian, &bytes[offset..offset + T::SIZE])
        })
    }

    /// Iterate over voxels in logical order (X varies fastest)
    pub fn iter_logical(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.header.file_endian();
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
    pub fn as_slice(&self) -> Result<&[T], Error>
    where
        T: bytemuck::Pod,
    {
        if !self.header.is_native_endian() {
            return Err(Error::EndiannessMismatch { detected: true });
        }

        if !self.header.axis_map().is_standard() {
            return Err(Error::NonContiguous);
        }

        let bytes = self.storage.as_ref();
        let expected_len = self.len();
        let expected_bytes = expected_len * T::SIZE;

        bytemuck::try_cast_slice(&bytes[..expected_bytes]).map_err(|_| Error::MisalignedData {
            required: core::mem::align_of::<T>(),
            actual: bytes.as_ptr().align_offset(core::mem::align_of::<T>()),
        })
    }

    /// Try zero-copy slice, fall back to decoded Vec on failure
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

        if self.header.is_native_endian() && self.header.axis_map().is_standard() {
            if let Ok(slice) = self.as_slice() {
                out[..n].copy_from_slice(&slice[..n]);
                return Ok(());
            }
        }

        let bytes = self.storage.as_ref();
        for (i, dst) in out[..n].iter_mut().enumerate() {
            let offset = i * T::SIZE;
            *dst = T::decode(self.header.file_endian(), &bytes[offset..offset + T::SIZE]);
        }

        Ok(())
    }

    /// Convert to owned Vec
    #[cfg(feature = "std")]
    pub fn to_vec(&self) -> alloc::vec::Vec<T>
    where
        T: bytemuck::Pod + Default,
    {
        if let Ok(slice) = self.as_slice() {
            return slice.to_vec();
        }
        self.iter().collect()
    }

    /// Convert linear index to logical coordinates (x, y, z)
    pub fn coords_of(&self, index: usize) -> (usize, usize, usize) {
        let (nx, ny, _) = self.dimensions();
        let z = index / (nx * ny);
        let remainder = index % (nx * ny);
        let y = remainder / nx;
        let x = remainder % nx;
        (x, y, z)
    }

    /// Convert logical coordinates to linear index
    pub fn linear_index(&self, x: usize, y: usize, z: usize) -> usize {
        x * self.strides[0] + y * self.strides[1] + z * self.strides[2]
    }

    /// Compute statistics (min, max, mean, rms) for the volume data
    pub fn stats(&self) -> crate::stats::Statistics
    where
        T: Into<f64>,
    {
        crate::stats::compute_stats(self.iter().map(|v| v.into()))
    }

    /// Extract a 2D slice from the volume at a specific Z position
    pub fn slice(&self, z: usize) -> Result<Slice2D<'_, T>, Error> {
        check_bounds(z, self.dimensions[2])?;

        let nx = self.dimensions[0];
        let ny = self.dimensions[1];
        let slice_size = nx * ny * T::SIZE;
        let slice_offset = z * nx * ny * T::SIZE;

        let bytes = self.storage.as_ref();
        let slice_data = &bytes[slice_offset..slice_offset + slice_size];

        Ok(Slice2D {
            data: slice_data,
            width: nx,
            height: ny,
            stride: nx,
            endian: self.header.file_endian(),
            _marker: core::marker::PhantomData,
        })
    }

    /// Extract a subvolume with the specified bounds
    #[cfg(feature = "std")]
    pub fn subvolume(
        &self,
        x_start: usize,
        x_end: usize,
        y_start: usize,
        y_end: usize,
        z_start: usize,
        z_end: usize,
    ) -> Result<Volume<T, Vec<u8>>, Error>
    where
        T: Default + Clone,
    {
        let validate_range = |start: usize, end: usize, dim: usize| -> Result<usize, Error> {
            if start >= dim || end > dim || start >= end {
                return Err(Error::IndexOutOfBounds {
                    index: start,
                    length: dim,
                });
            }
            Ok(end - start)
        };

        let new_nx = validate_range(x_start, x_end, self.dimensions[0])?;
        let new_ny = validate_range(y_start, y_end, self.dimensions[1])?;
        let new_nz = validate_range(z_start, z_end, self.dimensions[2])?;
        let voxel_count = new_nx * new_ny * new_nz;

        let mut new_data = vec![0u8; voxel_count * T::SIZE];

        (z_start..z_end).enumerate().for_each(|(new_z, src_z)| {
            (y_start..y_end).enumerate().for_each(|(new_y, src_y)| {
                (x_start..x_end).enumerate().for_each(|(new_x, src_x)| {
                    let src_idx = self.linear_index(src_x, src_y, src_z);
                    let dst_idx = new_z * new_ny * new_nx + new_y * new_nx + new_x;

                    let src_offset = src_idx * T::SIZE;
                    let dst_offset = dst_idx * T::SIZE;

                    new_data[dst_offset..dst_offset + T::SIZE]
                        .copy_from_slice(&self.storage.as_ref()[src_offset..src_offset + T::SIZE]);
                });
            });
        });

        let mut new_header = self.header.clone();
        new_header.set_dimensions(new_nx, new_ny, new_nz);
        let (dx, dy, dz) = new_header.voxel_size();
        new_header.set_origin(
            self.header.xorigin() + x_start as f32 * dx,
            self.header.yorigin() + y_start as f32 * dy,
            self.header.zorigin() + z_start as f32 * dz,
        );

        Volume::new(new_header, new_data)
    }
}

impl<T: Voxel + Encoding, S: AsMut<[u8]>> Volume<T, S> {
    /// Get mutable access to raw bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.storage.as_mut()
    }

    /// Set a voxel at 3D coordinates
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn set_at(&mut self, x: usize, y: usize, z: usize, value: T) {
        let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
        let offset = index * T::SIZE;
        let bytes = self.storage.as_mut();
        value.encode(
            self.header.file_endian(),
            &mut bytes[offset..offset + T::SIZE],
        );
    }

    /// Set a voxel at 3D coordinates, returning false if out of bounds
    pub fn set_at_opt(&mut self, x: usize, y: usize, z: usize, value: T) -> bool {
        if x >= self.dimensions[0] || y >= self.dimensions[1] || z >= self.dimensions[2] {
            return false;
        }
        self.set_at(x, y, z, value);
        true
    }

    /// Set a voxel at 3D coordinates, returning error if out of bounds
    pub fn set_at_checked(&mut self, x: usize, y: usize, z: usize, value: T) -> Result<(), Error> {
        if x >= self.dimensions[0] || y >= self.dimensions[1] || z >= self.dimensions[2] {
            let index = x * self.strides[0] + y * self.strides[1] + z * self.strides[2];
            return Err(Error::IndexOutOfBounds {
                index,
                length: self.len(),
            });
        }
        self.set_at(x, y, z, value);
        Ok(())
    }

    /// Extract a mutable 2D slice from the volume at a specific Z position
    pub fn slice_mut(&mut self, z: usize) -> Result<Slice2DMut<'_, T>, Error> {
        check_bounds(z, self.dimensions[2])?;

        let nx = self.dimensions[0];
        let ny = self.dimensions[1];
        let slice_size = nx * ny * T::SIZE;
        let slice_offset = z * nx * ny * T::SIZE;

        let bytes = self.storage.as_mut();
        let slice_data = &mut bytes[slice_offset..slice_offset + slice_size];

        Ok(Slice2DMut {
            data: slice_data,
            width: nx,
            height: ny,
            stride: nx,
            endian: self.header.file_endian(),
            _marker: core::marker::PhantomData,
        })
    }
}

// ============================================================================
// Slice2D - 2D view into a volume
// ============================================================================

/// Borrowed 2D slice view into a 3D volume
///
/// This is a zero-copy view - the data is not owned.
/// For owned 2D data, extract a subvolume with nz=1.
#[derive(Debug)]
pub struct Slice2D<'a, T: Voxel> {
    data: &'a [u8],
    width: usize,
    height: usize,
    stride: usize,
    endian: FileEndian,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Voxel + Encoding> Slice2D<'_, T> {
    /// Get dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Get width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get height
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get pixel at (x, y)
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get(&self, x: usize, y: usize) -> T {
        let index = y * self.stride + x;
        let offset = index * T::SIZE;
        T::decode(self.endian, &self.data[offset..offset + T::SIZE])
    }

    /// Get pixel with bounds check
    pub fn get_opt(&self, x: usize, y: usize) -> Option<T> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.get(x, y))
    }

    /// Get pixel with full error context
    pub fn get_checked(&self, x: usize, y: usize) -> Result<T, Error> {
        if x >= self.width || y >= self.height {
            return Err(Error::IndexOutOfBounds {
                index: y * self.stride + x,
                length: self.width * self.height,
            });
        }
        Ok(self.get(x, y))
    }

    /// Iterate over pixels in row-major order
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.endian;
        let stride = self.stride;
        let width = self.width;
        let height = self.height;
        let data = self.data;

        (0..height).flat_map(move |y| {
            (0..width).map(move |x| {
                let index = y * stride + x;
                let offset = index * T::SIZE;
                T::decode(endian, &data[offset..offset + T::SIZE])
            })
        })
    }

    /// Iterate over rows
    pub fn rows(&self) -> impl Iterator<Item = impl Iterator<Item = T> + '_> + '_ {
        let endian = self.endian;
        let stride = self.stride;
        let width = self.width;
        let data = self.data;

        (0..self.height).map(move |y| {
            let row_start = y * stride * T::SIZE;
            (0..width).map(move |x| {
                let offset = row_start + x * T::SIZE;
                T::decode(endian, &data[offset..offset + T::SIZE])
            })
        })
    }

    /// Convert to owned bytes (allocates)
    #[cfg(feature = "std")]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.data.to_vec()
    }
}

// ============================================================================
// Slice2DMut - Mutable 2D view into a volume
// ============================================================================

/// Mutable borrowed 2D slice view into a 3D volume
///
/// This is a zero-copy mutable view - the data is not owned.
#[derive(Debug)]
pub struct Slice2DMut<'a, T: Voxel> {
    data: &'a mut [u8],
    width: usize,
    height: usize,
    stride: usize,
    endian: FileEndian,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Voxel + Encoding> Slice2DMut<'_, T> {
    /// Get dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Get width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get height
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get pixel at (x, y)
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get(&self, x: usize, y: usize) -> T {
        let index = y * self.stride + x;
        let offset = index * T::SIZE;
        T::decode(self.endian, &self.data[offset..offset + T::SIZE])
    }

    /// Get pixel with bounds check
    pub fn get_opt(&self, x: usize, y: usize) -> Option<T> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.get(x, y))
    }

    /// Get pixel with full error context
    pub fn get_checked(&self, x: usize, y: usize) -> Result<T, Error> {
        if x >= self.width || y >= self.height {
            return Err(Error::IndexOutOfBounds {
                index: y * self.stride + x,
                length: self.width * self.height,
            });
        }
        Ok(self.get(x, y))
    }

    /// Set pixel at (x, y)
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn set(&mut self, x: usize, y: usize, value: T) {
        let index = y * self.stride + x;
        let offset = index * T::SIZE;
        value.encode(self.endian, &mut self.data[offset..offset + T::SIZE]);
    }

    /// Set pixel with bounds check, returns false if out of bounds
    pub fn set_opt(&mut self, x: usize, y: usize, value: T) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.set(x, y, value);
        true
    }

    /// Set pixel with full error context
    pub fn set_checked(&mut self, x: usize, y: usize, value: T) -> Result<(), Error> {
        if x >= self.width || y >= self.height {
            return Err(Error::IndexOutOfBounds {
                index: y * self.stride + x,
                length: self.width * self.height,
            });
        }
        self.set(x, y, value);
        Ok(())
    }

    /// Fill entire slice with a value
    pub fn fill(&mut self, value: T)
    where
        T: Copy,
    {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set(x, y, value);
            }
        }
    }

    /// Iterate over pixels in row-major order
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let endian = self.endian;
        let stride = self.stride;
        let width = self.width;
        let height = self.height;
        let data = &*self.data;

        (0..height).flat_map(move |y| {
            (0..width).map(move |x| {
                let index = y * stride + x;
                let offset = index * T::SIZE;
                T::decode(endian, &data[offset..offset + T::SIZE])
            })
        })
    }
}

// ============================================================================
// VolumeBuilder
// ============================================================================

/// Builder for constructing volumes
#[derive(Debug)]
pub struct VolumeBuilder<T: Voxel> {
    dimensions: [usize; 3],
    voxel_size: [f32; 3],
    origin: [f32; 3],
    cell_angles: [f32; 3],
    statistics: Option<(f32, f32, f32, f32)>,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Voxel + Encoding> VolumeBuilder<T> {
    /// Create a new volume builder
    pub fn new() -> Self {
        Self {
            dimensions: [1, 1, 1],
            voxel_size: [1.0, 1.0, 1.0],
            origin: [0.0, 0.0, 0.0],
            cell_angles: [90.0, 90.0, 90.0],
            statistics: None,
            _marker: core::marker::PhantomData,
        }
    }

    /// Set dimensions (nx, ny, nz)
    pub fn dimensions(mut self, nx: usize, ny: usize, nz: usize) -> Self {
        self.dimensions = [nx, ny, nz];
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

    /// Set statistics (min, max, mean, rms)
    pub fn statistics(mut self, dmin: f32, dmax: f32, dmean: f32, rms: f32) -> Self {
        self.statistics = Some((dmin, dmax, dmean, rms));
        self
    }

    /// Build with pre-existing storage
    pub fn build<S: AsRef<[u8]>>(self, storage: S) -> Result<Volume<T, S>, Error> {
        let mut header = Header::builder()
            .dimensions(self.dimensions[0], self.dimensions[1], self.dimensions[2])
            .mode(T::MODE)
            .cell_dimensions(
                self.voxel_size[0] * self.dimensions[0] as f32,
                self.voxel_size[1] * self.dimensions[1] as f32,
                self.voxel_size[2] * self.dimensions[2] as f32,
            )
            .origin(self.origin[0], self.origin[1], self.origin[2])
            .cell_angles(
                self.cell_angles[0],
                self.cell_angles[1],
                self.cell_angles[2],
            )
            .build();

        if let Some((dmin, dmax, dmean, rms)) = self.statistics {
            header.set_statistics(dmin, dmax, dmean, rms);
        }

        Volume::new(header, storage)
    }

    /// Build and allocate storage
    ///
    /// Returns an error if the total size would overflow.
    #[cfg(feature = "std")]
    pub fn build_allocated(self) -> Result<Volume<T, Vec<u8>>, Error> {
        let voxel_count = self.dimensions[0]
            .checked_mul(self.dimensions[1])
            .and_then(|v| v.checked_mul(self.dimensions[2]))
            .ok_or(Error::InvalidDimensions)?;

        let byte_size = voxel_count
            .checked_mul(T::SIZE)
            .ok_or(Error::InvalidDimensions)?;

        let data = vec![0u8; byte_size];
        self.build(data)
    }
}

impl<T: Voxel + Encoding> Default for VolumeBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Implement VolumeAccess trait
// ============================================================================

use super::traits::{VolumeAccess, VolumeAccessMut};

impl<T: Voxel + Encoding, S: AsRef<[u8]>> VolumeAccess for Volume<T, S> {
    type Voxel = T;

    fn header(&self) -> &Header {
        &self.header
    }

    fn dimensions(&self) -> (usize, usize, usize) {
        (self.dimensions[0], self.dimensions[1], self.dimensions[2])
    }

    fn strides(&self) -> (usize, usize, usize) {
        (self.strides[0], self.strides[1], self.strides[2])
    }

    fn len(&self) -> usize {
        self.dimensions[0] * self.dimensions[1] * self.dimensions[2]
    }

    fn get_at(&self, x: usize, y: usize, z: usize) -> T {
        self.get_at(x, y, z)
    }

    unsafe fn get_unchecked(&self, index: usize) -> T {
        let offset = index * T::SIZE;
        let bytes = self.storage.as_ref();
        T::decode(self.header.file_endian(), &bytes[offset..offset + T::SIZE])
    }
}

impl<T: Voxel + Encoding, S: AsRef<[u8]> + AsMut<[u8]>> VolumeAccessMut for Volume<T, S> {
    fn set_at(&mut self, x: usize, y: usize, z: usize, value: T) {
        self.set_at(x, y, z, value);
    }
}

// ============================================================================
// IntoIterator implementation
// ============================================================================

/// Iterator over volume voxels (storage order)
pub struct VolumeIntoIter<T: Voxel + Encoding> {
    data: Vec<u8>,
    endian: FileEndian,
    index: usize,
    len: usize,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Voxel + Encoding> Iterator for VolumeIntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            let offset = self.index * T::SIZE;
            self.index += 1;
            Some(T::decode(self.endian, &self.data[offset..offset + T::SIZE]))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<T: Voxel + Encoding> ExactSizeIterator for VolumeIntoIter<T> {}

impl<T: Voxel + Encoding> IntoIterator for Volume<T, Vec<u8>> {
    type Item = T;
    type IntoIter = VolumeIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.dimensions[0] * self.dimensions[1] * self.dimensions[2];
        VolumeIntoIter {
            data: self.storage,
            endian: self.header.file_endian(),
            index: 0,
            len,
            _marker: core::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_axis_map_strides() {
        let axis_map = AxisMap::new(1, 2, 3);
        let dimensions = [64, 64, 64];
        let strides = axis_map.strides(dimensions);
        assert_eq!(strides, [1, 64, 4096]);
    }

    #[test]
    fn test_nonstandard_axis_map_strides() {
        let axis_map = AxisMap::new(3, 2, 1);
        let dimensions = [64, 64, 64];
        let strides = axis_map.strides(dimensions);
        assert_eq!(strides, [4096, 64, 1]);
    }
}
