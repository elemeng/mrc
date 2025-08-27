#![no_std]
#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "f16")]
extern crate half;

mod header;
mod mode;
mod view;

#[cfg(test)]
#[path = "../test/tests.rs"]
mod tests;

pub use header::Header;
pub use mode::Mode;
pub use view::{MrcView, MrcViewMut};

// Optional file features
#[cfg(feature = "file")]
mod mrcfile;
#[cfg(test)]
#[cfg(feature = "file")]
#[path = "../test/mrcfile_test.rs"]
mod mrcfile_test;

#[cfg(feature = "mmap")]
pub use mrcfile::{MrcMmap, open_mmap};

#[cfg(feature = "file")]
pub use mrcfile::{MrcFile, open_file};

// Error type

#[derive(Debug)]
pub enum Error {
    Io,
    InvalidHeader,
    InvalidMode,
    InvalidDimensions,
    TypeMismatch,
    #[cfg(feature = "mmap")]
    Mmap,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Io => write!(f, "IO error"),
            Error::InvalidHeader => write!(f, "Invalid MRC header"),
            Error::InvalidMode => write!(f, "Invalid MRC mode"),
            Error::InvalidDimensions => write!(f, "Invalid dimensions"),
            Error::TypeMismatch => write!(f, "Type mismatch"),
            #[cfg(feature = "mmap")]
            Error::Mmap => write!(f, "Memory mapping error"),
        }
    }
}

#[cfg(feature = "std")]
impl core::error::Error for Error {}
