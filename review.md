  API Design Review (Post-Refactor Round 2)

  After reviewing the current codebase, the API is now significantly cleaner. Here are the remaining issues I've
  identified:

  ---

  1. MrcReader::ext_header_parsed() Clones Every Call

  Issue: Method clones extended header bytes on every call:

   pub fn ext_header_parsed(&self) -> ExtendedHeader {
       ExtendedHeader::from_bytes(&self.header.exttyp(), self.ext_header.clone())
   }

  Impact: Inefficient if called multiple times.

  Recommendation: Return a reference or make the intent clear by naming:

   /// Get parsed extended header (clones data)
   pub fn ext_header(&self) -> ExtendedHeader {
       ExtendedHeader::from_bytes(&self.header.exttyp(), self.ext_header.clone())
   }

   /// Get raw extended header bytes
   pub fn ext_header_bytes(&self) -> &[u8] {
       &self.ext_header
   }

  ---

  1. Volume Missing set_at_opt Inherent Method

  Issue: Volume has get_at_opt but not set_at_opt:

   // Has:
   pub fn get_at_opt(&self, x: usize, y: usize, z: usize) -> Option<T>
   pub fn set_at_checked(&mut self, x: usize, y: usize, z: usize, value: T) -> Result<(), Error>

   // Missing:
   // pub fn set_at_opt(&mut self, x: usize, y: usize, z: usize, value: T) -> bool

  Impact: Asymmetric API between read and write.

  Recommendation: Add set_at_opt:

   pub fn set_at_opt(&mut self, x: usize, y: usize, z: usize, value: T) -> bool {
       if x < self.dimensions[0] && y < self.dimensions[1] && z < self.dimensions[2] {
           self.set_at(x, y, z, value);
           true
       } else {
           false
       }
   }

  ---

  1. Slice2D Missing set_at Methods

  Issue: Slice2D is read-only; no way to modify pixel values:

   // Slice2D has:
   pub fn get(&self, x: usize, y: usize) -> T

   // No:
   // pub fn set(&mut self, x: usize, y: usize, value: T)

  Impact: Can't modify 2D slice views even when the underlying volume is mutable.

  Recommendation: Add Slice2DMut or make Slice2D generic over mutability. Or simply document that slices are
  read-only views (current design is defensible for zero-copy).

  ---

  1. MrcWriterBuilder::build Doesn't Return Writer

  Issue: The builder writes directly and returns Result<(), Error>:

   pub fn build(self, path: impl AsRef<std::path::Path>) -> Result<(), Error>

  Impact: Can't chain additional operations; inconsistent with other builders.

  Recommendation: Two-phase approach:

   impl MrcWriterBuilder {
       /// Create writer (doesn't write data yet)
       pub fn build(self, path: impl AsRef<std::path::Path>) -> Result<MrcWriter, Error> {
           // ... creates file with header
       }

       /// Convenience: create and write in one step
       pub fn write(self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
           // ... current behavior
       }
   }

  ---

  1. Encoding Trait Exposed But Methods Use unsafe

  Issue: The trait is public but has unsafe methods:

   pub trait Encoding: Voxel {
       const SIZE: usize;
       unsafe fn decode_unchecked(endian: FileEndian, bytes: &[u8]) -> Self;
       unsafe fn encode_unchecked(self, endian: FileEndian, bytes: &mut [u8]);
       fn decode(endian: FileEndian, bytes: &[u8]) -> Self { ... }
       fn encode(self, endian: FileEndian, bytes: &mut [u8]) { ... }
   }

  Impact: Users could call unsafe methods directly.

  Recommendation: Document clearly that users should use safe decode/encode methods. The unchecked variants are for
  internal use. Current design is acceptable.

  ---

  1. VolumeIter Not Exported But Used in Trait

  Issue: VolumeIter is exported but users rarely need it directly:

   pub use access::{..., VolumeIter, ...};

  Impact: Clutters public API.

  Recommendation: Remove from public exports. Users get iterators via volume.iter().

  ---

  1. Statistics Has Public Fields

  Issue: Fields are public:

   pub struct Statistics {
       pub min: f64,
       pub max: f64,
       pub mean: f64,
       pub rms: f64,
   }

  Impact: No invariants can be enforced; acceptable for a simple data struct.

  Recommendation: This is idiomatic for simple data transfer objects. No change needed.

  ---

  1. MrcSink::write_volume Redundant with MrcWriter::write_volume

  Issue: Same method exists on both trait and concrete type:

   // On MrcSink trait
   fn write_volume<T: Voxel + Encoding>(&mut self, volume: &Volume<T, Vec<u8>>) -> Result<(), Error>

   // On MrcWriter (via trait impl)
   // Same method available

  Impact: Slight duplication but acceptable - trait is for extensibility.

  Recommendation: No change needed; trait allows custom sinks.

  ---

  1. MrcReader Has No read_into Method

  Issue: Users must allocate a Vec even if they have their own buffer:

   let data = reader.read_data()?;  // Always allocates

  Impact: Can't reuse buffers; extra allocations.

  Recommendation: Add method for reading into existing buffer:

   pub fn read_into(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
       if buffer.len() < self.data_size {
           return Err(Error::BufferTooSmall { ... });
       }
       self.file.seek(SeekFrom::Start(self.data_offset))?;
       self.file.read_exact(&mut buffer[..self.data_size])?;
       Ok(())
   }

  ---

  1. Header::set_* Methods Return &mut Self

  Issue: Some setters return Result<&mut Self, Error>, others return &mut Self:
  

   pub fn set_dimensions(&mut self, nx: usize, ny: usize, nz: usize) -> &mut Self  // Infallible
   pub fn set_axis_map(&mut self, mapc: i32, mapr: i32, maps: i32) -> Result<&mut Self, Error>  // Fallible

  Impact: Inconsistent ergonomics.

  Recommendation: This is actually correct - validation requires fallibility. No change needed.
