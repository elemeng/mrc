#![no_std]
#[cfg(feature = "std")]
extern crate alloc;

mod header;
mod mode;
#[cfg(feature = "std")]
mod mrcfile;
#[cfg(test)]
#[cfg(feature = "std")]
#[path = "../test/mrcfile_test.rs"]
mod mrcfile_test;
#[cfg(test)]
#[path = "../test/tests.rs"]
mod tests;
mod view;

pub use header::Header;
pub use mode::Mode;
pub use view::{MrcView, MrcViewMut};

#[cfg(feature = "std")]
pub use mrcfile::{MrcFile, MrcMmap, open_file, open_mmap};

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
