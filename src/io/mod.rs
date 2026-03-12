//! IO operations for MRC files

#[cfg(feature = "std")]
pub mod traits;

#[cfg(feature = "std")]
mod reader;

#[cfg(feature = "std")]
mod writer;

#[cfg(feature = "std")]
pub use reader::MrcReader;
#[cfg(feature = "std")]
pub use writer::{MrcWriter, MrcWriterBuilder};

#[cfg(feature = "std")]
pub use traits::{MrcSink, MrcSource};
