#[cfg(feature = "std")]
use crate::{Error, Header, MrcView};

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::{fs::File, os::unix::fs::FileExt};

#[cfg(feature = "std")]
/// MrcFile for file I/O operations with pread/pwrite
pub struct MrcFile {
    file: File,
    header: Header,
    data_offset: u64,
    data_size: usize,
    ext_header_size: usize,
    buffer: alloc::vec::Vec<u8>,
}

#[cfg(feature = "std")]
impl MrcFile {
    #[inline]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let file = File::open(path).map_err(|_| Error::Io)?;
        let header = Self::read_header(&file)?;

        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let ext_header_size = header.nsymbt as usize;
        let data_offset = header.data_offset() as u64;
        let data_size = header.data_size();
        let total_size = ext_header_size + data_size;

        // Read all data into buffer
        let mut buffer = alloc::vec![0u8; total_size];
        if ext_header_size > 0 {
            file.read_exact_at(&mut buffer[..ext_header_size], 1024)
                .map_err(|_| Error::Io)?;
        }
        file.read_exact_at(&mut buffer[ext_header_size..], data_offset)
            .map_err(|_| Error::Io)?;

        Ok(Self {
            file,
            header,
            data_offset,
            data_size,
            ext_header_size,
            buffer,
        })
    }

    #[inline]
    pub fn create(path: impl AsRef<std::path::Path>, header: Header) -> Result<Self, Error> {
        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let file = File::create(path).map_err(|_| Error::Io)?;

        // Write the header using safe encode method
        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        file.write_all_at(&header_bytes, 0).map_err(|_| Error::Io)?;

        // Write extended header (zeros if none)
        let ext_header_size = header.nsymbt as usize;
        if ext_header_size > 0 {
            let zeros = alloc::vec![0u8; ext_header_size];
            file.write_all_at(&zeros, 1024).map_err(|_| Error::Io)?;
        }

        let data_offset = header.data_offset() as u64;
        let data_size = header.data_size();
        let total_size = ext_header_size + data_size;

        // Initialize buffer with zeros
        let buffer = alloc::vec![0u8; total_size];

        Ok(Self {
            file,
            header,
            data_offset,
            data_size,
            ext_header_size,
            buffer,
        })
    }

    #[inline]
    fn read_header(file: &File) -> Result<Header, Error> {
        let mut header_bytes = [0u8; 1024];
        file.read_exact_at(&mut header_bytes, 0)
            .map_err(|_| Error::Io)?;

        // Use safe decode method that handles endianness automatically
        let header = Header::decode_from_bytes(&header_bytes);

        Ok(header)
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    #[inline]
    /// Returns a combined view of the MRC file containing header, extended header, and data.
    ///
    /// This is a convenience method that provides access to all file components through
    /// a single `MrcView` object. The view internally splits the buffer into extended
    /// header and data based on the header's `nsymbt` field.
    ///
    /// # Example
    /// ```ignore
    /// let file = MrcFile::open("file.mrc")?;
    /// let view = file.read_view()?;
    ///
    /// // Access header information
    /// let (nx, ny, nz) = view.dimensions();
    ///
    /// // Access extended header (if present)
    /// let ext_header = view.ext_header();
    ///
    /// // Access main data block
    /// let data = view.data();
    /// ```
    pub fn read_view(&self) -> Result<MrcView<'_>, Error> {
        MrcView::new(self.header, &self.buffer)
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn write_view(&mut self, view: &MrcView) -> Result<(), Error> {
        // Write header using safe encode method
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        self.file
            .write_all_at(&header_bytes, 0)
            .map_err(|_| Error::Io)?;

        // Write extended header
        if self.ext_header_size > 0 {
            self.file
                .write_all_at(view.ext_header(), 1024)
                .map_err(|_| Error::Io)?;
        }

        // Write data
        self.file
            .write_all_at(view.data(), self.data_offset)
            .map_err(|_| Error::Io)?;

        // Update buffer with new data
        let total_size = self.ext_header_size + self.data_size;
        self.buffer.clear();
        self.buffer.resize(total_size, 0);
        self.buffer[..self.ext_header_size].copy_from_slice(view.ext_header());
        self.buffer[self.ext_header_size..].copy_from_slice(view.data());

        Ok(())
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn read_ext_header(&self) -> Result<&[u8], Error> {
        Ok(&self.buffer[..self.ext_header_size])
    }

    #[inline]
    #[allow(dead_code)] // Used in tests and public API
    pub fn write_ext_header(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.ext_header_size {
            return Err(Error::InvalidDimensions);
        }

        self.file.write_all_at(data, 1024).map_err(|_| Error::Io)?;
        self.buffer[..self.ext_header_size].copy_from_slice(data);
        Ok(())
    }

    #[inline]
    #[allow(dead_code)] // Used in tests and public API
    pub fn read_data(&self) -> Result<&[u8], Error> {
        Ok(&self.buffer[self.ext_header_size..])
    }

    #[inline]
    #[allow(dead_code)] // Used in tests and public API
    pub fn write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.data_size {
            return Err(Error::InvalidDimensions);
        }

        self.file
            .write_all_at(data, self.data_offset)
            .map_err(|_| Error::Io)?;
        self.buffer[self.ext_header_size..].copy_from_slice(data);
        Ok(())
    }
}

#[cfg(feature = "mmap")]
/// MrcMmap for memory-mapped file access
pub struct MrcMmap {
    header: Header,
    buffer: memmap2::Mmap,
    ext_header_size: usize,
    data_offset: usize,
    data_size: usize,
}

#[cfg(feature = "mmap")]
impl MrcMmap {
    #[inline]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        use memmap2::MmapOptions;

        let file = File::open(path).map_err(|_| Error::Io)?;
        let _metadata = file.metadata().map_err(|_| Error::Io)?;

        let buffer = unsafe { MmapOptions::new().map(&file).map_err(|_| Error::Io)? };

        if buffer.len() < 1024 {
            return Err(Error::InvalidHeader);
        }

        // Use safe decode method that handles endianness automatically
        let mut header_bytes = [0u8; 1024];
        header_bytes.copy_from_slice(&buffer[..1024]);
        let header = Header::decode_from_bytes(&header_bytes);

        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let ext_header_size = header.nsymbt as usize;
        let data_offset = header.data_offset();
        let data_size = header.data_size();

        if buffer.len() < data_offset + data_size {
            return Err(Error::InvalidDimensions);
        }

        Ok(Self {
            header,
            buffer,
            ext_header_size,
            data_offset,
            data_size,
        })
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    /// Returns a combined view of the MRC file containing header, extended header, and data.
    ///
    /// This is a convenience method that provides access to all file components through
    /// a single `MrcView` object. The view internally splits the memory-mapped buffer
    /// into extended header and data based on the header's `nsymbt` field.
    pub fn read_view(&self) -> Result<MrcView<'_>, Error> {
        // MrcView expects ext_header + data in contiguous buffer
        // For mmap, we can return a view that spans both regions
        let start = 1024;
        let end = self.data_offset + self.data_size;
        MrcView::new(self.header, &self.buffer[start..end])
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn ext_header(&self) -> &[u8] {
        if self.ext_header_size > 0 {
            &self.buffer[1024..1024 + self.ext_header_size]
        } else {
            &[]
        }
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn data(&self) -> &[u8] {
        &self.buffer[self.data_offset..self.data_offset + self.data_size]
    }
}

#[cfg(feature = "std")]
/// Compatibility functions
pub fn open_file(path: &str) -> Result<MrcFile, Error> {
    MrcFile::open(path)
}

#[cfg(feature = "std")]
#[allow(dead_code)] // Public API, may not be used in tests
pub fn save_file(path: &str, header: &Header, data: &[u8]) -> Result<(), Error> {
    let mut file = MrcFile::create(path, *header)?;
    file.write_data(data)?;
    Ok(())
}

#[cfg(feature = "mmap")]
pub fn open_mmap(path: &str) -> Result<MrcMmap, Error> {
    MrcMmap::open(path)
}
