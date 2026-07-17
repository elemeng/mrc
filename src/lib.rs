//! Read and write MRC-2014 files — the standard in cryo-EM and structural
//! biology. Handles byte-order, type conversion, and compression so you
//! can focus on your science. SIMD-accelerated, mmap-enabled.
//!
//! See the [README](https://github.com/elemeng/mrc#readme) for installation.
//! The companion [`mrc-cli`](https://crates.io/crates/mrc-cli) crate provides
//! a command-line tool for inspection, validation, conversion, and export.
//!
//! # Quick example
//!
//! ```no_run
//! use mrc::{read_as, write_as};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Read an entire volume in one call
//! let (header, data): (_, Vec<f32>) = read_as("density.mrc")?;
//! println!("{}×{}×{} volume, {} voxels",
//!     header.nx, header.ny, header.nz, data.len());
//!
//! // Write a volume in one call
//! write_as("output.mrc", &data, [512, 512, 256])?;
//! # Ok(()) }
//! ```
//!
//! # Reading files
//!
//! Open any MRC file with [`open()`] or [`Reader::open`]. Compression is
//! detected from magic bytes — no need to hint gzip or bzip2.
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::Reader;
//! let reader = Reader::open("tilt_series.mrc")?;
//! println!("{}×{}×{} voxels, mode {:?}",
//!     reader.shape().nx, reader.shape().ny, reader.shape().nz,
//!     reader.mode());
//! # Ok(()) }
//! ```
//!
//! Then pick an iteration method, each returning [`DataBlock`] chunks
//! whose [`DataView`] variant is determined by the file's mode — no
//! compile-time type guessing, no `ModeMismatch` errors at runtime:
//!
//! * [`slices`](Reader::slices) — one Z-plane at a time
//! * [`slabs`](Reader::slabs) — batches of `k` Z-planes
//! * [`tiles`](Reader::tiles) — arbitrary 3D blocks
//! * [`volumes`](Reader::volumes) — sub-volumes in a volume stack
//! * [`subregion`](Reader::subregion) — a single block by coordinate
//! * [`read_volume`](Reader::read_volume) — the entire volume as one block
//!
//! The file's voxel type is known at runtime via [`reader.mode()`](Reader::mode):
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! for slice in reader.slices() {
//!     let block = slice?;
//!     match block.data() {
//!         mrc::DataView::Float32(data) => { /* process f32 slice */ }
//!         mrc::DataView::Int16(data)   => { /* process i16 slice */ }
//!         mrc::DataView::Uint16(data)  => { /* process u16 slice */ }
//!         mrc::DataView::Int8(data)    => { /* process i8 slice */ }
//!         other                        => panic!("unhandled mode: {:?}", other),
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! Or just use [`convert::<f32>()`](Reader::convert) — the **fire-and-forget**
//! option that reads any mode as `f32`:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! for slice in reader.convert::<f32>().slices() {
//!     let block = slice?;
//!     println!("z={}: {} voxels", block.offset[2], block.data.len());
//! }
//! // Full volume in one call:
//! let block = reader.convert::<f32>().read_volume()?;
//! # Ok(()) }
//! ```
//!
//! The same iteration methods — [`slabs`](Reader::slabs),
//! [`tiles`](Reader::tiles), [`subregion`](Reader::subregion) — plus
//! [`with_complex_strategy`](crate::ConvertReader::with_complex_strategy) and
//! [`with_m0_interpretation`](crate::ConvertReader::with_m0_interpretation)
//! are all available on the returned converter.
//!
//! ### Special-mode reads (Packed4Bit and Mode 0)
//!
//! These modes have no [`Voxel`] implementation — there is no single Rust
//! type that maps to them safely at compile time — so dedicated methods read
//! directly as `u8` or `f32`:
//!
//! * [`slices_u8`](Reader::slices_u8) / [`slabs_u8`](Reader::slabs_u8) —
//!   unpack Packed4Bit nibbles, or narrow Uint16, to `u8`
//! * [`read_volume_u8`](Reader::read_volume_u8) — full volume as `u8`
//!   (Packed4Bit only)
//! * [`slices_mode0`](Reader::slices_mode0) / [`slabs_mode0`](Reader::slabs_mode0) —
//!   read Mode 0 as `f32`, choosing signed or unsigned interpretation
//!
//! ### Reading from memory or streams
//!
//! When your data is already in memory (e.g. from a camera readout, network
//! stream, or embedded resource), use [`Reader::from_reader`] or
//! [`Reader::from_bytes`]:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::Reader;
//! use std::io::Cursor;
//!
//! let bytes = std::fs::read("density.mrc")?;
//! let reader = Reader::from_reader(Cursor::new(bytes))?;
//! # Ok(()) }
//! ```
//!
//! Permissive variants [`Reader::from_reader_permissive`] and
//! [`Reader::from_bytes_permissive`] accept non-critical header issues as
//! warnings without failing, mirroring [`Reader::open_permissive`].
//!
//! ### Large files
//!
//! When the file does not fit in RAM, [`Reader::open`] automatically uses
//! memory-mapped I/O (requires the `mmap` feature). Same iterator API,
//! zero-copy [`DataBlock`] views, OS-managed paging.
//!
//! For buffered readers (in-memory buffers, compressed files), the default
//! reader methods also return zero-copy views when the requested block is a
//! native-endian contiguous full-row slab.
//!
//! ### Quirky files
//!
//! Common microscope quirks (NVERSION left at 0, `"MAP\0"` instead of `"MAP "`)
//! are handled transparently by [`open()`] — no special flags needed.
//!
//! For esoteric or severely non-standard files, use
//! [`Reader::open_permissive`] which turns non-critical header issues into
//! warnings instead of hard errors, and allows opening files that are shorter
//! than the header declares. Use [`is_truncated`](Reader::is_truncated) to
//! detect truncated data after a permissive open:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # use mrc::Reader;
//! let (reader, warnings) = Reader::open_permissive("legacy.mrc")?;
//! if reader.is_truncated() {
//!     eprintln!("warning: file is incomplete");
//! }
//! for w in &warnings { eprintln!("note: {w}"); }
//! # Ok(()) }
//! ```
//!
//! # Writing files
//!
//! Use [`create()`] to get a [`WriterBuilder`], set the shape and voxel type,
//! then call [`finish`](WriterBuilder::finish).
//!
//! For the simplest case — write an entire volume — use [`write_as()`] or
//! [`Writer::set_data`]:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::write_as;
//! let data = vec![0.0f32; 256 * 256 * 128];
//! write_as("output.mrc", &data, [256, 256, 128])?;
//! # Ok(()) }
//! ```
//!
//! For streaming writes (one slice at a time), use the builder API:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::{create, VoxelBlock};
//! let mut writer = create("output.mrc")
//!     .shape([256, 256, 128])
//!     .mode::<f32>()
//!     .finish()?;
//!
//! // Write some data
//! writer.write_block(&VoxelBlock::new(
//!     [0, 0, 0], [256, 256, 1],
//!     vec![0.0f32; 256 * 256],
//! )?)?;
//!
//! // Optionally compute and store density statistics
//! writer.update_header_stats()?;
//!
//! // Finalize — required, rewrites header with final metadata
//! writer.finalize()?;
//! # Ok(()) }
//! ```
//!
//! The lifecycle:
//!
//! 1. **Write** blocks with [`write_block`](Writer::write_block). The type
//!    `T` matches the file's mode — a compile-time check that prevents
//!    accidentally treating bytes as the wrong kind of number.
//!    Use [`write_block_as`](Writer::write_block_as) for automatic conversion
//!    (e.g. write `f32` data to an Int16 or Float16 file)
//!
//!    For Uint16 files, [`write_u8_block`](Writer::write_u8_block) auto-widens
//!    `u8` data; for Packed4Bit files, [`write_u4_block`](Writer::write_u4_block)
//!    packs `u8` values (0–15) two-per-byte.
//! * [`convert_u8_slice_to_u16`] — widen `&[u8]` to `Vec<u16>` for writing to
//!   Uint16 files (used internally by [`write_u8_block`](Writer::write_u8_block))
//! * [`convert_u16_slice_to_u8`] — narrow `&[u16]` to `Vec<u8>` (returns `Err`
//!   if any value exceeds 255)
//! * [`reinterpret_m0`] — reinterpret Mode 0 data as signed or unsigned `f32`
//!
//! 2. Optionally call [`update_header_stats`](Writer::update_header_stats)
//!    to fill in `dmin`/`dmax`/`dmean`/`rms`.
//! 3. **Finalize** with [`finalize`](Writer::finalize) to rewrite the header
//!    with final metadata. **Required** — without it the header is stale.
//!
//! Four backends through the same builder, plus in-memory output:
//!
//! | Backend | Builder method | Best for |
//! |---|---|---|
//! | [`Writer`] | [`finish()`](WriterBuilder::finish) | General use, writes straight to disk |
//! | [`Writer`] (in-memory) | [`finish_buffer()`](WriterBuilder::finish_buffer) | Memory buffer, e.g. testing or in-memory processing |
//! | [`Writer`] (mmap) | [`finish_mmap()`](WriterBuilder::finish_mmap) | Very large files (`mmap` feature) |
//! | [`Writer`] (gzip) | [`finish_gzip()`](WriterBuilder::finish_gzip) | Compressed output (`gzip` feature) |
//! | [`Writer`] (bzip2) | [`finish_bzip2()`](WriterBuilder::finish_bzip2) | Compressed output (`bzip2` feature) |
//!
//! > **Note:** The builder is the recommended path. The lower-level
//! > [`Writer::from_writer`], [`Writer::from_writer_mmap`],
//! > [`Writer::from_writer_gzip`], and [`Writer::from_writer_bzip2`]
//! > constructors are also available for callers who already have a
//! > [`Header`] or a custom I/O target — see their respective docs for details.
//!
//! Compressed writers support configurable [`CompressionLevel`] level via
//! [`WriterBuilder::compression`]:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::{CompressionLevel, create};
//!
//! let mut writer = create("output.mrc.gz")
//!     .shape([256, 256, 128])
//!     .mode::<f32>()
//!     .compression(CompressionLevel::Best)
//!     .finish_gzip()?;
//! # Ok(()) }
//! ```
//!
//! # Data modes
//!
//! MRC files encode voxels in one of several numeric modes. [`Mode`]
//! represents them at runtime; [`Voxel`] ties each Rust type to its mode
//! at compile time, catching mismatches before any data flows.
//!
//! | Mode | Rust type | Typical use |
//! |---|---|---|
//! | [`Int8`](Mode::Int8) (0) | `i8` | Binary masks |
//! | [`Int16`](Mode::Int16) (1) | `i16` | Raw cryo-EM density |
//! | [`Float32`](Mode::Float32) (2) | `f32` | Processed / reconstructed density |
//! | [`Int16Complex`](Mode::Int16Complex) (3) | [`Int16Complex`] | Complex data (i16 real + i16 imag) |
//! | [`Float32Complex`](Mode::Float32Complex) (4) | [`Float32Complex`] | Complex data (f32 real + f32 imag) |
//! | [`Uint16`](Mode::Uint16) (6) | `u16` | Segmentation labels |
//! | [`Float16`](Mode::Float16) (12) | `f16` | Half-precision storage (feature `f16`) |
//! | [`Packed4Bit`](Mode::Packed4Bit) (101) | `u8` via [`slices_u8`](Reader::slices_u8) | 4-bit packed data; no `Voxel` impl |
//!
//! Packed 4-bit data is handled transparently by the unified API:
//! [`convert::<f32>()`](Reader::convert) unpacks nibbles to `f32`,
//! [`slices_u8`](Reader::slices_u8) / [`slabs_u8`](Reader::slabs_u8) unpack
//! to `u8` (0–15), and [`write_u4_block`](Writer::write_u4_block) packs
//! `u8` values back.
//!
//! When you don't know the mode ahead of time, use
//! [`convert::<f32>()`](Reader::convert) which converts any mode to `f32`.
//!
//! # Feature flags
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `mmap` | Memory-mapped readers and writers | ✅ |
//! | `f16` | Half-precision float via the `half` crate | ✅ |
//! | `simd` | AVX2 / NEON acceleration for integer↔f32, f16↔f32, byte-swap, stats, and f32→integer clamping | ✅ |
//! | `parallel` | Parallel encoding via `rayon` | ✅ |
//! | `gzip` | Gzip-compressed I/O | ✅ |
//! | `bzip2` | Bzip2-compressed I/O | ❌ |
//! | `ndarray` | Return volumes as `ndarray::Array3<T>` via `to_ndarray()` | ❌ |
//! | `serde` | Serialize/Deserialize support via `serde` | ❌ |
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("density.mrc")?;
//! # #[cfg(feature = "ndarray")]
//! # {
//! use ndarray::Array3;
//! let arr: Array3<f32> = reader.convert::<f32>().to_ndarray()?;
//! println!("{}×{}×{} array", arr.shape()[0], arr.shape()[1], arr.shape()[2]);
//! # }
//! # Ok(()) }
//! ```
//!
//! # Headers
//!
//! The [`Header`] struct mirrors the 1024-byte MRC-2014 fixed header.
//! Every field is a typed public member — dimensions, cell parameters,
//! axis mapping, density statistics, text labels, and more.
//!
//! ```
//! use mrc::Header;
//! let h = Header::new();
//! assert_eq!(h.map, *b"MAP ");
//! ```
//!
//! For fluent construction with validation, use [`HeaderBuilder`]:
//!
//! ```
//! use mrc::HeaderBuilder;
//! let header = HeaderBuilder::new()
//!     .shape([512, 512, 256])
//!     .mode::<f32>()
//!     .cell_lengths(1.0, 1.0, 1.0)
//!     .cell_angles(90.0, 90.0, 90.0)
//!     .origin([0.0, 0.0, 0.0])
//!     .nstart([0, 0, 0])
//!     .sampling([512, 512, 256])
//!     .axis_mapping([1, 2, 3])
//!     .add_label("reconstructed map")
//!     .build()?;
//! # Ok::<_, mrc::HeaderValidationError>(())
//! ```
//!
//! Three validation levels:
//!
//! * [`validate`](Header::validate) — quick yes / no
//! * [`validate_detailed`](Header::validate_detailed) — tells you exactly
//!   what is wrong via [`HeaderValidationError`]
//! * [`validate_permissive`](Header::validate_permissive) — warnings for
//!   non-critical issues
//!
//! ### Volume type helpers
//!
//! The header distinguishes four volume types. Configure them explicitly
//! when creating files that are not single 3D volumes:
//!
//! ```rust
//! # use mrc::Header;
//! let mut h = Header::new();
//! h.nx = 64; h.ny = 64; h.nz = 120;
//! h.mx = 64; h.my = 64; h.mz = 30;
//! h.set_volume_stack(30); // ispg = 401, mz = 30
//! assert!(h.is_volume_stack());
//! assert_eq!(h.logical_shape(), [4, 30, 64, 64]); // 4 sub-volumes
//! ```
//!
//! ### Convenience API
//!
//! The [`Header`] provides computed properties for common queries:
//!
//! | Method | Returns | Description |
//! |---|---|---|
//! | [`validate()`](Header::validate) | `bool` | Quick validity check |
//! | [`validate_detailed()`](Header::validate_detailed) | `Result<(), HeaderValidationError>` | Full structural validation with diagnostics |
//! | [`validate_permissive()`](Header::validate_permissive) | `Result<Vec<String>>` | Lenient validation, returns warnings |
//! | [`is_single_image()`](Header::is_single_image) | `bool` | `nz == 1` |
//! | [`is_image_stack()`](Header::is_image_stack) | `bool` | `ispg == 0` |
//! | [`is_volume()`](Header::is_volume) | `bool` | Not a stack and not an image stack |
//! | [`is_volume_stack()`](Header::is_volume_stack) | `bool` | `ispg` in 401–630 |
//! | [`set_image_stack()`](Header::set_image_stack) | `()` | Set as image stack (`ispg = 0`, `mz = 1`) |
//! | [`set_volume()`](Header::set_volume) | `()` | Set as single volume (`ispg = 1`, `mz = nz`) |
//! | [`set_volume_stack(mz)`](Header::set_volume_stack) | `()` | Set as volume stack with sub-volume size `mz` |
//! | [`logical_shape()`](Header::logical_shape) | `[usize; 4]` | `[nvolumes, mz, ny, nx]` |
//! | [`exttyp()`](Header::exttyp) | `[u8; 4]` | Extended header type from `extra[8..12]` |
//! | [`exttyp_str()`](Header::exttyp_str) | `Result<&str>` | Extended header type as string (UTF-8 decoded) |
//! | [`nversion()`](Header::nversion) | `i32` | NVERSION from `extra[12..16]` |
//! | [`get_labels()`](Header::get_labels) | `Vec<String>` | Read up to `nlabl` non-empty labels |
//! | [`label_at(i)`](Header::label_at) | `Option<&str>` | Trimmed label at index `i`, or `None` if empty |
//! | [`add_label(text)`](Header::add_label) | `()` | Append a text label (FIFO when full) |
//! | [`density_stats()`](Header::density_stats) | `(f32, f32, f32, f32)` | `(dmin, dmax, dmean, rms)` |
//! | [`sampling()`](Header::sampling) | `[i32; 3]` | `[mx, my, mz]` |
//! | [`voxel_size()`](Header::voxel_size) | `[f32; 3]` | Å/pixel = `cella / mxyz` |
//! | [`cell_lengths()`](Header::cell_lengths) | `[f32; 3]` | `[xlen, ylen, zlen]` |
//! | [`cell_angles()`](Header::cell_angles) | `[f32; 3]` | `[alpha, beta, gamma]` |
//! | [`cell_volume()`](Header::cell_volume) | `f64` | Unit cell volume in Å³ (triclinic formula) |
//! | [`nstart()`](Header::nstart) | `[i32; 3]` | `[nxstart, nystart, nzstart]` |
//! | [`detect_endian()`](Header::detect_endian) | `FileEndian` | Detect byte order from MACHST |
//! | [`set_file_endian(endian)`](Header::set_file_endian) | `()` | Set MACHST and re-encode NVERSION |
//! | [`is_standard_map()`](Header::is_standard_map) | `bool` | MAP field is exactly `"MAP "` |
//! | [`detect_imod()`](Header::detect_imod) | `Option<ImodInfo>` | Detect IMOD stamp in `extra` bytes |
//! | [`is_y_inverted()`](Header::is_y_inverted) | `bool` | `true` when `mapr == -2` (IMOD convention) |
//! | [`decode_from_bytes(bytes)`](Header::decode_from_bytes) | `Header` | Parse from raw 1024 bytes (auto endian) |
//! | [`decode_from_bytes_with_info(bytes)`](Header::decode_from_bytes_with_info) | `(Header, Option<EndianFallbackWarning>)` | Parse with endian fallback diagnostics |
//! | [`encode_to_bytes(&mut [u8; 1024])`](Header::encode_to_bytes) | `()` | Encode to raw bytes |
//!
//! ### Manual header parsing
//!
//! Decode a raw 1024-byte header block with automatic endianness detection:
//!
//! ```rust
//! use mrc::Header;
//! let raw = [0u8; 1024];
//! // ... fill raw bytes from file ...
//! let header = Header::decode_from_bytes(&raw);
//! ```
//!
//! When the MACHST byte-order stamp is wrong (common in some EPU files),
//! the decoder tries the opposite endianness automatically:
//!
//! ```rust
//! # use mrc::Header;
//! # let raw = [0u8; 1024];
//! let (header, warning) = Header::decode_from_bytes_with_info(&raw);
//! if let Some(w) = warning {
//!     eprintln!("byte-order fallback used: {w}");
//! }
//! ```
//!
//! # Extended headers
//!
//! Many MRC files carry additional metadata after the 1024-byte fixed header
//! in an **extended header** region. The type is identified by the 4-byte
//! `exttyp` field in the header's `extra[8..12]`.
//!
//! The [`ExtHeaderType`] enum identifies the format without parsing:
//!
//! ```rust
//! use mrc::{Header, ExtHeaderType};
//! let header = Header::new();
//! match ExtHeaderType::from_header(&header) {
//!     ExtHeaderType::Fei1 => println!("FEI Type 1"),
//!     ExtHeaderType::Ccp4 => println!("CCP4"),
//!     ExtHeaderType::Unknown(id) => {
//!         println!("Unknown: {:?}", std::str::from_utf8(&id));
//!     }
//!     _ => {}
//! }
//! ```
//!
//! Instead of calling individual parser functions, use the auto-dispatch
//! method on any open reader:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("file.mrc")?;
//! use mrc::ExtHeaderData;
//!
//! match reader.parse_extended_header() {
//!     ExtHeaderData::Fei1(records) => {
//!         println!("FEI1 tilt series ({} records)", records.len());
//!         for r in &records {
//!             println!("  tilt {:.1}°, defocus {:.1} µm",
//!                 r.alpha_tilt, r.defocus);
//!         }
//!     }
//!     ExtHeaderData::Ccp4(records) => {
//!         println!("CCP4 symmetry ({} records)", records.len());
//!     }
//!     ExtHeaderData::Seri(records) => {
//!         println!("  first tilt: {:.1}°", records[0].alpha_tilt);
//!     }
//!     ExtHeaderData::None => println!("No recognized extended header"),
//!     _ => {}
//! }
//! # Ok(()) }
//! ```
//!
//! Typed convenience methods give direct access without pattern matching:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("file.mrc")?;
//! if let Some(records) = reader.fei1_metadata() {
//!     println!("{} FEI1 records", records.len());
//! }
//! if let Some(imod) = reader.imod_metadata() {
//!     println!("IMOD type {:?}, tilt increment {:.1}°",
//!         imod.image_type, imod.tilt_increment);
//! }
//! # Ok(()) }
//! ```
//!
//! Available: [`fei1_metadata`](crate::Reader::fei1_metadata),
//! [`fei2_metadata`](crate::Reader::fei2_metadata),
//! [`ccp4_records`](crate::Reader::ccp4_records),
//! [`mrco_records`](crate::Reader::mrco_records),
//! [`seri_records`](crate::Reader::seri_records),
//! [`agar_records`](crate::Reader::agar_records),
//! [`imod_metadata`](crate::Reader::imod_metadata).
//!
//! # Error handling
//!
//! Fallible functions return `Result<T, Error>`. Match on specific variants
//! to handle different failure modes:
//!
//! ```rust
//! use mrc::Error;
//!
//! fn describe(err: &Error) -> &str {
//!     match err {
//!         Error::Io(_) => "I/O failure",
//!         Error::InvalidHeader => "not a valid MRC file",
//!         Error::ModeMismatch { .. } => "wrong voxel type for this file",
//!         Error::BoundsError { .. } => "block outside volume",
//!         Error::FileSizeMismatch { .. } => "file truncated or has trailing data",
//!         _ => "other",
//!     }
//! }
//! ```
//!
//! The errors you will actually hit in practice:
//!
//! * [`Io`](Error::Io) — the file could not be read or written
//! * [`InvalidHeader`](Error::InvalidHeader) — not a valid MRC file
//! * [`ModeMismatch`](Error::ModeMismatch) — writing a `VoxelBlock<i16>` to
//!   a Float32 file; use [`write_block_as`](Writer::write_block_as) instead
//! * [`BoundsError`](Error::BoundsError) — read or write outside the volume
//! * [`NotAVolumeStack`](Error::NotAVolumeStack) — calling [`volumes()`](Reader::volumes)
//!   on a file that is not a volume stack
//! * [`FileSizeMismatch`](Error::FileSizeMismatch) — file truncated or
//!   has trailing garbage
//!
//! [`HeaderValidationError`] gives fine-grained diagnostics for header
//! problems (bad dimensions, wrong MAP field, invalid NVERSION ...).
//!
//! # Endianness
//!
//! MRC files encode byte order via a 4-byte MACHST stamp at file offset 212.
//! [`FileEndian`] represents the detected byte order:
//!
//! ```rust
//! use mrc::FileEndian;
//! let stamp: [u8; 4] = [0x44, 0x44, 0x00, 0x00]; // little-endian
//! assert_eq!(FileEndian::from_machst(&stamp), FileEndian::LittleEndian);
//! ```
//!
//! Use [`FileEndian::native`] to query the host platform, and
//! [`reader.endian()`](Reader::endian) to get a file's actual byte order.
//! New files are always little-endian, matching modern hardware and the Python
//! `mrcfile` library.
//!
//! The crate has a fallback: if the MODE field is invalid under the detected
//! endianness, the opposite byte order is tried. This handles files with a
//! wrong MACHST stamp but correct data.
//!
//! # Compression auto-detection
//!
//! [`Reader::open`] reads the first two bytes of the file:
//!
//! | Magic bytes | Format |
//! |---|---|
//! | `\x1f\x8b` | Gzip |
//! | `BZ` | Bzip2 |
//! | anything else | Plain |
//!
//! Plain MRC files are memory-mapped or buffered directly. Compressed files
//! are fully decompressed into memory on open, with a hard cap of
//! [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB) to prevent bombs.
//! Use [`Reader::open_gzip_with_limit`] or
//! [`Reader::open_bzip2_with_limit`] for a custom limit.
//!
//! > **Large compressed files:** If the uncompressed data exceeds available RAM,
//! > decompress with `gunzip` or `bunzip2` first, then use [`Reader::open`] for
//! > zero-copy access — the OS pages data on demand without loading the whole
//! > file into memory.
//!
//! # File validation
//!
//! [`validate_full`](validate::validate_full) runs comprehensive checks
//! on a file — header, size, endianness, data statistics (1% tolerance),
//! and NaN / Inf scanning. Returns a
//! [`ValidationReport`](validate::ValidationReport) with categorized issues:
//!
//! ```no_run
//! use mrc::validate::{validate_full, Severity};
//!
//! let report = validate_full("protein.mrc", false)?;
//! if !report.is_valid() {
//!     for issue in &report.issues {
//!         if issue.severity == Severity::Error {
//!             eprintln!("[{}] {}", issue.category, issue.message);
//!         }
//!     }
//! }
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```
//!
//! If you already have an open [`Reader`], use
//! [`validate_reader`](validate::validate_reader) to avoid re-opening
//! the file.
//!
//! # Real-world workflows
//!
//! ## 1. Process a tilt series
//!
//! A common cryo-EM workflow: open a tilt series, read the FEI metadata,
//! then iterate over slices:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! use mrc::open;
//!
//! let reader = open("tiltseries.mrc")?;
//! println!("{}×{}×{} voxels, mode {:?}",
//!     reader.shape().nx, reader.shape().ny, reader.shape().nz,
//!     reader.mode());
//!
//! // Read FEI extended header metadata via convenience method
//! if let Some(records) = reader.fei1_metadata() {
//!     for (i, r) in records.iter().enumerate() {
//!         println!("tilt {i}: α={:.1}°, defocus={:.1} µm",
//!             r.alpha_tilt, r.defocus);
//!     }
//! }
//!
//! // Process each slice
//! for slice in reader.convert::<f32>().slices() {
//!     let block = slice?;
//!     // block.data: Vec<f32> — ready for filtering, CTF correction, etc.
//! }
//! # Ok(()) }
//! ```
//!
//! If a file fails to open, try [`open_permissive`](Reader::open_permissive)
//! for lenient header handling, or [`validate_full`](validate::validate_full)
//! to diagnose the issue.
//!
//! ## 2. Write a processed map
//!
//! Always call [`finalize`](Writer::finalize) — without it the header is
//! stale and density statistics will be wrong (tools display wrong contrast).
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use mrc::create;
//!
//! let mut writer = create("reconstructed.mrc")
//!     .shape([512, 512, 256])
//!     .mode::<f32>()
//!     .finish()?;
//!
//! for z in 0..256 {
//!     let slice = vec![0.0f32; 512 * 512];
//!     writer.write_block(&mrc::VoxelBlock::new(
//!         [0, 0, z], [512, 512, 1], slice,
//!     )?)?;
//! }
//!
//! writer.update_header_stats()?;
//! writer.finalize()?;
//! # Ok(()) }
//! ```
//!
//! ## 3. Read subtomogram averages from a volume stack
//!
//! Volume stacks (ISPG 401–630) pack multiple sub-volumes into one file,
//! each `mz` slices thick. Use [`volumes`](Reader::volumes) to iterate:
//!
//! ```no_run
//! # fn main() -> Result<(), mrc::Error> {
//! # let reader = mrc::Reader::open("averages.mrc")?;
//! for volume in reader.volumes()? {
//!     let vol = volume?;
//!     println!("sub-volume at z={} ({}×{}×{} voxels)",
//!         vol.offset()[2], vol.shape()[0], vol.shape()[1], vol.shape()[2]);
//! }
//! # Ok(()) }
//! ```
//!
//! # Troubleshooting
//!
//! | Error | Likely cause | What to try |
//! |---|---|---|
//! | [`InvalidHeader`](Error::InvalidHeader) | Not an MRC file, or header corruption | Run `mrc validate file.mrc`; try [`open_permissive`](Reader::open_permissive) |
//! | [`FileSizeMismatch`](Error::FileSizeMismatch) | File truncated or has trailing garbage | Re-download or check `mrc validate` output |
//! | [`ModeMismatch`](Error::ModeMismatch) | Writing a `VoxelBlock<i16>` to an Float32 file | Use [`write_block_as`](Writer::write_block_as) — auto-converts any mode |
//! | [`NotAVolumeStack`](Error::NotAVolumeStack) | Calling `volumes()` on a non-stack file | Check `reader.is_volume_stack()` first |
//! | [`BoundsError`](Error::BoundsError) | Block outside volume | Check offset + shape against dimensions |
//! | [`BlockShapeMismatch`](Error::BlockShapeMismatch) | Data length doesn't match block shape | Verify `sx * sy * sz * sizeof(T)` matches data length |
//! | [`UnsupportedMode`](Error::UnsupportedMode) | Unrecognized mode, or mode needs the `f16` feature | Enable `f16` feature or convert with another tool |
//! | `Io` error | File permissions, filesystem issue | Check the file path and permissions |
//! | Values look wrong | Endianness mismatch | The endianness fallback handles most cases; try `mrc validate` |
//!
//! # Philosophy
//!
//! This crate does **one thing** — read and write MRC files. It does no array
//! arithmetic, image processing, or type conversion beyond MRC-specific
//! shortcuts (`convert::<f32>()`, `slices_mode0`, `slices_u8`).
//! Leave those to crates like `ndarray`, or your own code.

#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::perf)
)]
#![warn(missing_docs, clippy::cargo)]

mod engine;
mod error;
mod header;
mod io;
mod iter;
mod mode;
pub mod validate;

#[cfg(feature = "serde")]
mod serde_byte_array;

// Re-export core types
pub use engine::block::{VolumeShape, VoxelBlock};
/// Endianness of MRC file data.
pub use engine::endian::FileEndian;

// Re-export MRC-specific format utilities
pub use engine::convert::{convert_u8_slice_to_u16, convert_u16_slice_to_u8, reinterpret_m0};

pub use error::{Error, HeaderValidationError};
pub use header::{
    AGAR_RECORD_SIZE, AgarRecord, CCP4_RECORD_SIZE, Ccp4Record, ExtHeaderData, ExtHeaderType,
    FEI1_RECORD_SIZE, FEI2_RECORD_SIZE, Fei1Metadata, Fei2Metadata, Header, HeaderBuilder,
    ImodImageType, ImodInfo, ImodMetadata, MRCO_RECORD_SIZE, MrcoRecord, SERI_RECORD_SIZE,
    SeriRecord, parse_agar_records, parse_ccp4_records, parse_fei1_records, parse_fei2_records,
    parse_imod_metadata, parse_mrco_records, parse_seri_records,
};

pub use mode::{
    ComplexToRealStrategy, DataBlock, DataView, Float32Complex, Int16Complex, M0Interpretation,
    Mode, OwnedData, Voxel,
};

/// Half-precision floating point type (requires `f16` feature).
#[cfg(feature = "f16")]
pub use half::f16;
/// Consolidated MRC reader with automatic mmap/buffered backend selection.
pub use io::reader::Reader;

/// Auto-conversion wrapper returned by [`Reader::convert`].
pub use io::reader_common::ConvertReader;

/// MRC file writer and its builder.
pub use io::writer::{Writer, WriterBuilder};

/// Compression level for compressed MRC writers.
///
/// See [`WriterBuilder::compression`] for usage.
pub use io::writer::CompressionLevel;

/// Default decompression safety limit for gzip/bzip2 files (256 GiB).
///
/// Applied before the header is parsed, preventing decompression bombs.
/// Override via [`Reader::open_gzip_with_limit`] or
/// [`Reader::open_bzip2_with_limit`].
pub use io::reader_common::DEFAULT_MAX_DECOMPRESSED_BYTES;

#[doc(hidden)]
pub use engine::codec::{decode_into, swap_bytes_in_place};

#[doc(hidden)]
pub use io::reader::{CompressionType, detect_compression};

/// Internal helper trait for [`read_as`] — users do not need to interact with it directly.
///
/// All standard voxel types (`f32`, `i16`, `u16`, `i8`, etc.) implement this trait.
#[doc(hidden)]
pub trait ReadAsTarget: Voxel + crate::engine::convert::ConvertFrom<f32> {}
impl<T: Voxel + crate::engine::convert::ConvertFrom<f32>> ReadAsTarget for T {}

/// Open an MRC file for reading, auto-detecting gzip or bzip2 compression.
///
/// This is a convenience wrapper around [`Reader::open`].
/// Common microscope quirks (NVERSION left at 0, `"MAP\0"` instead of `"MAP "`)
/// are handled transparently — no special flags needed.
///
/// For compressed files, decompression is capped at
/// [`DEFAULT_MAX_DECOMPRESSED_BYTES`] (256 GiB) to prevent bombs.
/// Use [`Reader::open_gzip_with_limit`] or [`Reader::open_bzip2_with_limit`]
/// to set a custom limit.
///
/// For permissive mode (returns `(Reader, Vec<String>)` instead of
/// `Reader`), or compressed-file-specific openers,
/// use [`Reader::open_permissive`], [`Reader::open_gzip`], etc. directly.
pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Reader, Error> {
    Reader::open(path)
}

/// Create a new MRC file for writing.
///
/// Returns a [`WriterBuilder`] that must be configured with at least
/// [`shape`](WriterBuilder::shape) and [`mode`](WriterBuilder::mode)
/// before calling [`finish`](WriterBuilder::finish) to open the file.
///
/// # Example
/// ```no_run
/// use mrc::create;
///
/// let mut writer = create("output.mrc")
///     .shape([256, 256, 128])
///     .mode::<f32>()
///     .finish()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn create<P: AsRef<std::path::Path>>(path: P) -> WriterBuilder {
    WriterBuilder::new(path)
}

/// Read an entire MRC volume into a `Vec<T>` with auto-mode detection.
///
/// This is a one-shot convenience over manually opening a [`Reader`] and
/// calling [`convert::<T>()`](Reader::convert) then [`read_volume`](Reader::read_volume).
/// The file can be in any MRC mode — the data is auto-converted to `T`.
///
/// Returns the parsed [`Header`] and the voxel data.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use mrc::read_as;
/// let (header, data): (_, Vec<f32>) = read_as("density.mrc")?;
/// println!("{}×{}×{} volume, {} voxels",
///     header.nx, header.ny, header.nz, data.len());
/// # Ok(()) }
/// ```
pub fn read_as<T: ReadAsTarget, P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(Header, Vec<T>), Error> {
    let reader = Reader::open(path)?;
    let header = *reader.header();
    let volume = reader.convert::<T>().read_volume()?;
    Ok((header, volume.data))
}

/// Write an entire MRC volume from a `&[T]` with a single call.
///
/// Creates the file, writes the data, computes density statistics,
/// and finalizes — all in one step. The type `T` determines the
/// file's MRC mode (e.g. `f32` → Mode 2, `i16` → Mode 1).
///
/// # Examples
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use mrc::write_as;
/// let data = vec![0.0f32; 64 * 64 * 32];
/// write_as("output.mrc", &data, [64, 64, 32])?;
/// # Ok(()) }
/// ```
pub fn write_as<T: Voxel, P: AsRef<std::path::Path>>(
    path: P,
    data: &[T],
    shape: [usize; 3],
) -> Result<(), Error> {
    let mut writer = WriterBuilder::new(path).shape(shape).mode::<T>().finish()?;
    writer.set_data(data)?;
    writer.finalize()?;
    Ok(())
}
