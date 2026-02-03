#[cfg(feature = "std")]
use crate::{Error, Header, MrcView};
use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::fs::File;

#[cfg(feature = "std")]
/// MrcFile for file I/O operations
///
/// This struct provides file-based access to MRC files. Data is loaded into
/// memory when the file is opened. For zero-copy memory-mapped access to
/// large files, use [`MrcMmap`] instead.
#[derive(Debug)]
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
        let mut file = File::open(path).map_err(Error::Io)?;
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
            file.seek(SeekFrom::Start(1024)).map_err(Error::Io)?;
            file.read_exact(&mut buffer[..ext_header_size])
                .map_err(Error::Io)?;
        }
        {
            file.seek(SeekFrom::Start(data_offset)).map_err(Error::Io)?;
            file.read_exact(&mut buffer[ext_header_size..])
                .map_err(Error::Io)?;
        }

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

        let mut file = File::create(path).map_err(Error::Io)?;

        // Write the header using safe encode method
        let mut header_bytes = [0u8; 1024];
        header.encode_to_bytes(&mut header_bytes);
        {
            file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
            file.write_all(&header_bytes).map_err(Error::Io)?;
        }

        // Write extended header (zeros if none)
        let ext_header_size = header.nsymbt as usize;
        if ext_header_size > 0 {
            let zeros = alloc::vec![0u8; ext_header_size];
            {
                file.seek(SeekFrom::Start(1024)).map_err(Error::Io)?;
                file.write_all(&zeros).map_err(Error::Io)?;
            }
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
    fn read_header(mut file: &File) -> Result<Header, Error> {
        let mut header_bytes = [0u8; 1024];
        file.seek(SeekFrom::Start(0)).map_err(Error::Io)?; // move point to the beginning position that intended to be read
        file.read_exact(&mut header_bytes).map_err(Error::Io)?; // read is a stream operation at the current position

        // Use safe decode method that handles endianness automatically
        let header = Header::decode_from_bytes(&header_bytes);

        Ok(header)
    }

    #[inline]
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    #[inline]
    /// Returns a combined view of the MRC file containing header, extended header, and data.
    ///
    /// This method provides access to all file components through a single `MrcView` object.
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
        let ext_header = &self.buffer[..self.ext_header_size];
        let data = &self.buffer[self.ext_header_size..];
        MrcView::from_parts(self.header, ext_header, data)
    }

    #[inline]
    pub fn write_view(&mut self, view: &MrcView) -> Result<(), Error> {
        // Validate view dimensions match file dimensions
        if view.ext_header().len() != self.ext_header_size {
            return Err(Error::InvalidDimensions);
        }
        if view.data.as_bytes().len() != self.data_size {
            return Err(Error::InvalidDimensions);
        }

        // Write header using safe encode method
        let mut header_bytes = [0u8; 1024];
        self.header.encode_to_bytes(&mut header_bytes);
        {
            self.file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;
            self.file.write_all(&header_bytes).map_err(Error::Io)?;
        }

        // Write extended header
        if self.ext_header_size > 0 {
            {
                self.file.seek(SeekFrom::Start(1024)).map_err(Error::Io)?;
                self.file.write_all(view.ext_header()).map_err(Error::Io)?;
            }
        }

        // Write data
        {
            self.file
                .seek(SeekFrom::Start(self.data_offset))
                .map_err(Error::Io)?;
            self.file
                .write_all(view.data.as_bytes())
                .map_err(Error::Io)?;
        }

        Ok(())
    }

    #[inline]
    pub fn read_ext_header(&self) -> Result<&[u8], Error> {
        Ok(&self.buffer[..self.ext_header_size])
    }

    #[inline]
    pub fn write_ext_header(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.ext_header_size {
            return Err(Error::InvalidDimensions);
        }

        self.file.seek(SeekFrom::Start(1024)).map_err(Error::Io)?;
        self.file.write_all(data).map_err(Error::Io)?;
        self.buffer[..self.ext_header_size].copy_from_slice(data);
        Ok(())
    }

    #[inline]
    pub fn read_data(&self) -> Result<&[u8], Error> {
        Ok(&self.buffer[self.ext_header_size..])
    }

    #[inline]
    pub fn write_data(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != self.data_size {
            return Err(Error::InvalidDimensions);
        }

        self.file
            .seek(SeekFrom::Start(self.data_offset))
            .map_err(Error::Io)?;
        self.file.write_all(data).map_err(Error::Io)?;
        self.buffer[self.ext_header_size..].copy_from_slice(data);
        Ok(())
    }
}

#[cfg(feature = "mmap")]
/// MrcMmap for memory-mapped file access
///
/// This struct provides zero-copy access to MRC files using OS memory mapping.
/// It is ideal for large files where eager loading would consume too much memory.
///
/// # Safety
/// Memory mapping is performed using `unsafe` code internally (via the `memmap2`
/// crate), but the public API is safe. The mapped memory is valid for the lifetime
/// of this struct, and access is controlled through Rust's borrowing rules.
#[derive(Debug)]
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

        let file = File::open(path).map_err(Error::Io)?;

        let buffer = unsafe { MmapOptions::new().map(&file).map_err(Error::Io)? };

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

        // Check for potential overflow and sufficient buffer size
        let total_size = data_offset
            .checked_add(data_size)
            .ok_or(Error::InvalidDimensions)?;
        if buffer.len() < total_size {
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
    /// Returns a combined view of the MRC file containing header, extended header, and data.
    ///
    /// This method provides access to all file components through a single `MrcView` object.
    /// The memory-mapped buffer is explicitly separated into extended header and data sections.
    ///
    /// # Memory Layout
    /// ```text
    /// File layout:  | 1024 bytes | NSYMBT bytes | data_size bytes |
    ///               | Header     | ExtHeader    | VoxelData       |
    ///
    /// mmap buffer:  [0..1024)    [1024..data_offset) [data_offset..)
    /// ```
    pub fn read_view(&self) -> Result<MrcView<'_>, Error> {
        // Explicitly separate extended header and data from mmap buffer
        let ext_header = if self.ext_header_size > 0 {
            &self.buffer[1024..1024 + self.ext_header_size]
        } else {
            &[]
        };
        let data = &self.buffer[self.data_offset..self.data_offset + self.data_size];
        MrcView::from_parts(self.header, ext_header, data)
    }

    #[inline]
    pub fn ext_header(&self) -> &[u8] {
        if self.ext_header_size > 0 {
            &self.buffer[1024..1024 + self.ext_header_size]
        } else {
            &[]
        }
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.buffer[self.data_offset..self.data_offset + self.data_size]
    }
}

#[cfg(feature = "mmap")]
pub fn open_mmap(path: &str) -> Result<MrcMmap, Error> {
    MrcMmap::open(path)
}
