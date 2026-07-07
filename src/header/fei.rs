//! FEI1/FEI2 extended header structured parsing.
//!
//! The FEI extended header contains one metadata record per image section.
//! This module provides typed access to the most commonly used fields.
//! For fields not yet covered, use the raw byte slice directly.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Size of a single FEI1 metadata record, in bytes.
pub const FEI1_RECORD_SIZE: usize = 768;

/// Size of a single FEI2 metadata record, in bytes.
pub const FEI2_RECORD_SIZE: usize = 888;

/// Common FEI1 metadata fields.
///
/// Fields are parsed from big-endian bytes following the EPU/Thermo Fisher
/// MRC-2014 specification. Not all 185 fields are exposed; only the most
/// frequently used cryo-EM metadata is included. Access raw bytes for
/// unsupported fields.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub struct Fei1Metadata {
    pub metadata_size: u32,
    pub metadata_version: u32,
    pub bitmask_1: u32,
    pub timestamp: f64,
    pub microscope_type: [u8; 16],
    pub ht: f64,
    pub dose: f64,
    pub alpha_tilt: f64,
    pub beta_tilt: f64,
    pub x_stage: f64,
    pub y_stage: f64,
    pub z_stage: f64,
    pub tilt_axis_angle: f64,
    pub pixel_size_x: f64,
    pub pixel_size_y: f64,
    pub defocus: f64,
    pub stem_defocus: f64,
    pub applied_defocus: f64,
    pub magnification: f64,
    pub camera_length: f64,
    pub spot_index: i32,
    pub illuminated_area: f64,
    pub intensity: f64,
    pub convergence_angle: f64,
    pub slit_width: f64,
    pub shift_offset_x: f64,
    pub shift_offset_y: f64,
    pub shift_x: f64,
    pub shift_y: f64,
    pub integration_time: f64,
    pub binning_width: i32,
    pub binning_height: i32,
    pub camera_name: [u8; 16],
    pub readout_area_left: i32,
    pub readout_area_top: i32,
    pub readout_area_right: i32,
    pub readout_area_bottom: i32,
    pub ceta_frames_summed: i32,
    pub phase_plate: bool,
    pub gain: f64,
    pub offset: f64,
    pub dwell_time: f64,
    pub frame_time: f64,
    pub full_scan_fov_x: f64,
    pub full_scan_fov_y: f64,
    pub is_dose_fraction: bool,
    pub fraction_number: i32,
    pub start_frame: i32,
    pub end_frame: i32,
    pub alpha_tilt_min: f64,
    pub alpha_tilt_max: f64,
}

impl Fei1Metadata {
    /// Parse a single FEI1 record from bytes.
    ///
    /// Returns `None` if `bytes` is shorter than [`FEI1_RECORD_SIZE`].
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < FEI1_RECORD_SIZE {
            return None;
        }
        Some(Self {
            metadata_size: be_u32(bytes, 0),
            metadata_version: be_u32(bytes, 4),
            // bitmask_1 at offset 8 is stored as little-endian in the FEI
            // specification (unlike the rest of the record which is big-endian).
            bitmask_1: le_u32(bytes, 8),
            timestamp: be_f64(bytes, 12),
            microscope_type: read_bytes(bytes, 20),
            ht: be_f64(bytes, 84),
            dose: be_f64(bytes, 92),
            alpha_tilt: be_f64(bytes, 100),
            beta_tilt: be_f64(bytes, 108),
            x_stage: be_f64(bytes, 116),
            y_stage: be_f64(bytes, 124),
            z_stage: be_f64(bytes, 132),
            tilt_axis_angle: be_f64(bytes, 140),
            pixel_size_x: be_f64(bytes, 156),
            pixel_size_y: be_f64(bytes, 164),
            defocus: be_f64(bytes, 220),
            stem_defocus: be_f64(bytes, 228),
            applied_defocus: be_f64(bytes, 236),
            magnification: be_f64(bytes, 289),
            camera_length: be_f64(bytes, 301),
            spot_index: be_i32(bytes, 309),
            illuminated_area: be_f64(bytes, 313),
            intensity: be_f64(bytes, 321),
            convergence_angle: be_f64(bytes, 329),
            slit_width: be_f64(bytes, 355),
            shift_offset_x: be_f64(bytes, 387),
            shift_offset_y: be_f64(bytes, 395),
            shift_x: be_f64(bytes, 403),
            shift_y: be_f64(bytes, 411),
            integration_time: be_f64(bytes, 419),
            binning_width: be_i32(bytes, 427),
            binning_height: be_i32(bytes, 431),
            camera_name: read_bytes(bytes, 435),
            readout_area_left: be_i32(bytes, 451),
            readout_area_top: be_i32(bytes, 455),
            readout_area_right: be_i32(bytes, 459),
            readout_area_bottom: be_i32(bytes, 463),
            ceta_frames_summed: be_i32(bytes, 468),
            phase_plate: bytes[518] != 0,
            gain: be_f64(bytes, 535),
            offset: be_f64(bytes, 543),
            dwell_time: be_f64(bytes, 571),
            frame_time: be_f64(bytes, 579),
            full_scan_fov_x: be_f64(bytes, 603),
            full_scan_fov_y: be_f64(bytes, 611),
            is_dose_fraction: bytes[655] != 0,
            fraction_number: be_i32(bytes, 656),
            start_frame: be_i32(bytes, 660),
            end_frame: be_i32(bytes, 664),
            alpha_tilt_min: be_f64(bytes, 752),
            alpha_tilt_max: be_f64(bytes, 760),
        })
    }
}

/// FEI2 metadata extends FEI1 with additional v2 fields.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub struct Fei2Metadata {
    pub fei1: Fei1Metadata,
    pub scan_rotation: f64,
    pub diffraction_pattern_rotation: f64,
    pub image_rotation: f64,
    pub scan_mode_enumeration: i32,
    pub acquisition_time_stamp: i64,
    pub detector_commercial_name: [u8; 16],
    pub start_tilt_angle: f64,
    pub end_tilt_angle: f64,
    pub tilt_per_image: f64,
    pub tilt_speed: f64,
    pub beam_center_x_pixel: i32,
    pub beam_center_y_pixel: i32,
    pub cfeg_flash_timestamp: i64,
    pub phase_plate_position_index: i32,
    pub objective_aperture_name: [u8; 16],
}

impl Fei2Metadata {
    /// Parse a single FEI2 record from bytes.
    ///
    /// Returns `None` if `bytes` is shorter than [`FEI2_RECORD_SIZE`].
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < FEI2_RECORD_SIZE {
            return None;
        }
        let fei1 = Fei1Metadata::from_bytes(bytes)?;
        Some(Self {
            fei1,
            scan_rotation: be_f64(bytes, 768),
            diffraction_pattern_rotation: be_f64(bytes, 776),
            image_rotation: be_f64(bytes, 784),
            scan_mode_enumeration: be_i32(bytes, 792),
            acquisition_time_stamp: be_i64(bytes, 796),
            detector_commercial_name: read_bytes(bytes, 804),
            start_tilt_angle: be_f64(bytes, 820),
            end_tilt_angle: be_f64(bytes, 828),
            tilt_per_image: be_f64(bytes, 836),
            tilt_speed: be_f64(bytes, 844),
            beam_center_x_pixel: be_i32(bytes, 852),
            beam_center_y_pixel: be_i32(bytes, 856),
            cfeg_flash_timestamp: be_i64(bytes, 860),
            phase_plate_position_index: be_i32(bytes, 868),
            objective_aperture_name: read_bytes(bytes, 872),
        })
    }
}

/// Parse a raw extended header byte slice as a vector of FEI1 records.
///
/// Returns `None` if `bytes` is empty or if its length is not an exact
/// multiple of [`FEI1_RECORD_SIZE`].
pub fn parse_fei1_records(bytes: &[u8]) -> Option<Vec<Fei1Metadata>> {
    if bytes.is_empty() || bytes.len() % FEI1_RECORD_SIZE != 0 {
        return None;
    }
    let count = bytes.len() / FEI1_RECORD_SIZE;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * FEI1_RECORD_SIZE;
        records.push(Fei1Metadata::from_bytes(
            &bytes[start..start + FEI1_RECORD_SIZE],
        )?);
    }
    Some(records)
}

/// Parse a raw extended header byte slice as a vector of FEI2 records.
///
/// Returns `None` if `bytes` is empty or if its length is not an exact
/// multiple of [`FEI2_RECORD_SIZE`].
pub fn parse_fei2_records(bytes: &[u8]) -> Option<Vec<Fei2Metadata>> {
    if bytes.is_empty() || bytes.len() % FEI2_RECORD_SIZE != 0 {
        return None;
    }
    let count = bytes.len() / FEI2_RECORD_SIZE;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * FEI2_RECORD_SIZE;
        records.push(Fei2Metadata::from_bytes(
            &bytes[start..start + FEI2_RECORD_SIZE],
        )?);
    }
    Some(records)
}

// ============================================================================
// Little helper fns for big-endian parsing
// ============================================================================

#[inline]
fn be_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[inline]
fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[inline]
fn be_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[inline]
fn be_i64(bytes: &[u8], offset: usize) -> i64 {
    i64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

#[inline]
fn be_f64(bytes: &[u8], offset: usize) -> f64 {
    f64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

#[inline]
fn read_bytes<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let mut arr = [0u8; N];
    arr.copy_from_slice(&bytes[offset..offset + N]);
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic FEI1 record buffer with recognisable values.
    fn make_fei1_record() -> Vec<u8> {
        let mut buf = vec![0u8; FEI1_RECORD_SIZE];
        // metadata_size (offset 0, u32, big-endian)
        buf[0..4].copy_from_slice(&768u32.to_be_bytes());
        // metadata_version (offset 4, u32, big-endian)
        buf[4..8].copy_from_slice(&1u32.to_be_bytes());
        // bitmask_1 (offset 8, u32, little-endian — special case)
        buf[8..12].copy_from_slice(&42u32.to_le_bytes());
        // timestamp (offset 12, f64, big-endian)
        buf[12..20].copy_from_slice(&123_456.789_f64.to_be_bytes());
        // alpha_tilt (offset 100, f64, big-endian)
        buf[100..108].copy_from_slice(&(-35.5f64).to_be_bytes());
        // defocus (offset 220, f64, big-endian)
        buf[220..228].copy_from_slice(&(2.5f64).to_be_bytes());
        // ht (offset 84, f64, big-endian)
        buf[84..92].copy_from_slice(&300_000.0_f64.to_be_bytes());
        // dose (offset 92, f64, big-endian)
        buf[92..100].copy_from_slice(&50.0f64.to_be_bytes());
        // pixel_size_x (offset 156, f64, big-endian)
        buf[156..164].copy_from_slice(&1.34f64.to_be_bytes());
        // magnification (offset 289, f64, big-endian)
        buf[289..297].copy_from_slice(&47000.0f64.to_be_bytes());
        // spot_index (offset 309, i32, big-endian)
        buf[309..313].copy_from_slice(&7i32.to_be_bytes());
        // camera_name (offset 435, 16 bytes)
        buf[435..451].copy_from_slice(b"Falcon 4        ");
        // phase_plate (offset 518, bool)
        buf[518] = 1;
        // gain (offset 535, f64, big-endian)
        buf[535..543].copy_from_slice(&2.5f64.to_be_bytes());
        buf
    }

    #[test]
    fn parse_fei1_known_values() {
        let buf = make_fei1_record();
        let records = parse_fei1_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.metadata_size, 768);
        assert_eq!(r.metadata_version, 1);
        assert_eq!(r.bitmask_1, 42);
        assert!((r.timestamp - 123456.789).abs() < 1e-6);
        assert!((r.alpha_tilt - (-35.5)).abs() < 1e-6);
        assert!((r.defocus - 2.5).abs() < 1e-6);
        assert!((r.ht - 300000.0).abs() < 1e-6);
        assert!((r.dose - 50.0).abs() < 1e-6);
        assert!((r.pixel_size_x - 1.34).abs() < 1e-6);
        assert!((r.magnification - 47000.0).abs() < 1e-6);
        assert_eq!(r.spot_index, 7);
        assert_eq!(&r.camera_name[..7], b"Falcon ");
        assert!(r.phase_plate);
        assert!((r.gain - 2.5).abs() < 1e-6);
    }

    #[test]
    fn parse_fei1_multiple_records() {
        let mut buf = make_fei1_record();
        buf.extend_from_slice(&make_fei1_record());
        let records = parse_fei1_records(&buf).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn parse_fei1_empty_bytes() {
        assert!(parse_fei1_records(&[]).is_none());
    }

    #[test]
    fn parse_fei1_misaligned_length() {
        let buf = vec![0u8; FEI1_RECORD_SIZE + 1];
        assert!(parse_fei1_records(&buf).is_none());
    }

    #[test]
    fn parse_fei2_known_values() {
        let mut buf = vec![0u8; FEI2_RECORD_SIZE];
        // Fill FEI1 portion with recognisable values
        buf[0..4].copy_from_slice(&768u32.to_be_bytes());
        buf[4..8].copy_from_slice(&1u32.to_be_bytes());
        buf[100..108].copy_from_slice(&(-35.5f64).to_be_bytes());
        buf[220..228].copy_from_slice(&(2.5f64).to_be_bytes());
        // FEI2-specific fields
        buf[768..776].copy_from_slice(&(90.0f64).to_be_bytes()); // scan_rotation
        buf[796..804].copy_from_slice(&1234567890i64.to_be_bytes()); // acquisition_time_stamp
        buf[804..820].copy_from_slice(b"Falcon 4i       "); // detector_commercial_name

        let records = parse_fei2_records(&buf).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert!((r.scan_rotation - 90.0).abs() < 1e-6);
        assert_eq!(r.acquisition_time_stamp, 1234567890);
        assert_eq!(&r.detector_commercial_name[..9], b"Falcon 4i");
        // FEI1 fields should also be accessible
        assert!((r.fei1.alpha_tilt - (-35.5)).abs() < 1e-6);
        assert!((r.fei1.defocus - 2.5).abs() < 1e-6);
    }

    #[test]
    fn parse_fei2_short_buffer() {
        let buf = vec![0u8; FEI1_RECORD_SIZE]; // too short for FEI2
        assert!(parse_fei2_records(&buf).is_none());
    }
}
