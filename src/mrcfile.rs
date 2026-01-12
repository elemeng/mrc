#[cfg(feature = "std")]
use crate::{Error, Header, MrcView};

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::{boxed::Box, fs::File, os::unix::fs::FileExt};

#[cfg(feature = "std")]
/// MrcFile for file I/O operations with pread/pwrite
pub struct MrcFile {
    file: File,
    header: Header,
    data_offset: u64,
    data_size: usize,
    ext_header_size: usize,
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

        Ok(Self {
            file,
            header,
            data_offset,
            data_size,
            ext_header_size,
        })
    }

    #[inline]
    pub fn create(path: impl AsRef<std::path::Path>, header: Header) -> Result<Self, Error> {
        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let file = File::create(path).map_err(|_| Error::Io)?;

        // Write the header
        // Use safe serialization to avoid undefined behavior
        let mut header_bytes = [0u8; 1024];
        unsafe {
            // Copy header bytes safely to avoid alignment issues
            let src = &header as *const Header as *const u8;
            let dst = header_bytes.as_mut_ptr();
            core::ptr::copy_nonoverlapping(src, dst, 1024);
        }
        file.write_all_at(&header_bytes, 0).map_err(|_| Error::Io)?;

        // Write extended header (zeros if none)
        if header.nsymbt > 0 {
            let zeros = alloc::vec![0u8; header.nsymbt as usize];
            file.write_all_at(&zeros, 1024).map_err(|_| Error::Io)?;
        }

        let ext_header_size = header.nsymbt as usize;
        let data_offset = header.data_offset() as u64;
        let data_size = header.data_size();

        Ok(Self {
            file,
            header,
            data_offset,
            data_size,
            ext_header_size,
        })
    }

    #[inline]
    fn read_header(file: &File) -> Result<Header, Error> {
        let mut header_bytes = [0u8; 1024];
        file.read_exact_at(&mut header_bytes, 0)
            .map_err(|_| Error::Io)?;

        // Validate we have exactly 1024 bytes for the header
        if header_bytes.len() != 1024 {
            return Err(Error::InvalidHeader);
        }

        // Ensure proper alignment for Header type
        let header = unsafe {
            let ptr = header_bytes.as_ptr() as *const Header;
            // Check alignment before reading
            if (ptr as usize) % core::mem::align_of::<Header>() != 0 {
                // Use read_unaligned for potentially unaligned reads
                ptr.read_unaligned()
            } else {
                ptr.read()
            }
        };

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
    pub fn read_view(&self) -> Result<MrcView<'static>, Error> {
        let mut buffer = alloc::vec![0u8; self.ext_header_size + self.data_size];

        // Read extended header
        if self.ext_header_size > 0 {
            self.file
                .read_exact_at(&mut buffer[..self.ext_header_size], 1024)
                .map_err(|_| Error::Io)?;
        }

        // Read data
        self.file
            .read_exact_at(&mut buffer[self.ext_header_size..], self.data_offset)
            .map_err(|_| Error::Io)?;

        let buffer_slice = Box::leak(buffer.into_boxed_slice());
        MrcView::new(self.header, buffer_slice)
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn write_view(&mut self, view: &MrcView) -> Result<(), Error> {
        // Write header using safe serialization
        let mut header_bytes = [0u8; 1024];
        unsafe {
            // Copy header bytes safely to avoid alignment issues
            let src = &self.header as *const Header as *const u8;
            let dst = header_bytes.as_mut_ptr();
            core::ptr::copy_nonoverlapping(src, dst, 1024);
        }
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

        Ok(())
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn read_ext_header(&self) -> Result<Box<[u8]>, Error> {
        if self.ext_header_size == 0 {
            return Ok(Box::new([]));
        }

        let mut buffer = alloc::vec![0u8; self.ext_header_size];
        self.file
            .read_exact_at(&mut buffer, 1024)
            .map_err(|_| Error::Io)?;

        Ok(buffer.into_boxed_slice())
    }

    #[inline]
    #[allow(dead_code)] // Used in tests and public API
    pub fn write_ext_header(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.ext_header_size {
            return Err(Error::InvalidDimensions);
        }

        self.file.write_all_at(data, 1024).map_err(|_| Error::Io)?;
        Ok(())
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn read_data(&self) -> Result<&'static [u8], Error> {
        let mut buffer = alloc::vec![0u8; self.data_size];
        self.file
            .read_exact_at(&mut buffer, self.data_offset)
            .map_err(|_| Error::Io)?;

        let buffer_slice = Box::leak(buffer.into_boxed_slice());
        Ok(buffer_slice)
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
        Ok(())
    }
}

#[cfg(feature = "mmap")]
/// MrcMmap for memory-mapped file access
pub struct MrcMmap {
    header: Header,
    data: &'static [u8],
    ext_header: &'static [u8],
    _file: File,
}

#[cfg(feature = "mmap")]
impl MrcMmap {
    #[inline]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        use memmap2::MmapOptions;

        let file = File::open(path).map_err(|_| Error::Io)?;
        let _metadata = file.metadata().map_err(|_| Error::Io)?;

        let mmap = unsafe { MmapOptions::new().map(&file).map_err(|_| Error::Io)? };

        let buffer = Box::leak(Box::new(mmap));

        if buffer.len() < 1024 {
            return Err(Error::InvalidHeader);
        }

        // Ensure proper alignment and safe deserialization
        let header = unsafe {
            let ptr = buffer.as_ptr() as *const Header;
            // Always use read_unaligned for memory-mapped data
            ptr.read_unaligned()
        };

        if !header.validate() {
            return Err(Error::InvalidHeader);
        }

        let ext_header_size = header.nsymbt as usize;
        let data_offset = header.data_offset();
        let data_size = header.data_size();

        if buffer.len() < data_offset + data_size {
            return Err(Error::InvalidDimensions);
        }

        let ext_header = if ext_header_size > 0 {
            &buffer[1024..1024 + ext_header_size]
        } else {
            &[]
        };

        let data = &buffer[data_offset..data_offset + data_size];

        Ok(Self {
            header,
            data,
            ext_header,
            _file: file,
        })
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    pub fn read_view(&self) -> Result<MrcView<'static>, Error> {
        let mut buffer = alloc::vec![0u8; self.ext_header.len() + self.data.len()];
        buffer[..self.ext_header.len()].copy_from_slice(self.ext_header);
        buffer[self.ext_header.len()..].copy_from_slice(self.data);

        let buffer_slice = Box::leak(buffer.into_boxed_slice());
        MrcView::new(self.header, buffer_slice)
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn ext_header(&self) -> &[u8] {
        self.ext_header
    }

    #[inline]
    #[allow(dead_code)] // Public API, may not be used in tests
    pub fn data(&self) -> &[u8] {
        self.data
    }
}

#[cfg(feature = "std")]
/// Compatibility functions
pub fn open_file(path: &str) -> Result<MrcView<'static>, Error> {
    let file = MrcFile::open(path)?;
    file.read_view()
}

#[cfg(feature = "std")]
#[allow(dead_code)] // Public API, may not be used in tests
pub fn save_file(path: &str, header: &Header, data: &[u8]) -> Result<(), Error> {
    let mut file = MrcFile::create(path, *header)?;
    file.write_data(data)?;
    Ok(())
}

#[cfg(feature = "mmap")]
pub fn open_mmap(path: &str) -> Result<MrcView<'static>, Error> {
    let file = MrcMmap::open(path)?;
    file.read_view()
}
