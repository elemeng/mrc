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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error")]
    Io,
    #[error("Invalid MRC header")]
    InvalidHeader,
    #[error("Invalid MRC mode")]
    InvalidMode,
    #[error("Invalid dimensions")]
    InvalidDimensions,
    #[error("Type mismatch")]
    TypeMismatch,
    #[cfg(feature = "mmap")]
    #[error("Memory mapping error")]
    Mmap,
}
