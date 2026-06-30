//! MRC file validation infrastructure.
//!
//! Provides [`validate_full`] for comprehensive file validation and
//! [`ValidationReport`] for structured results.

use crate::engine::stats::compute_stats;
use crate::engine::endian::FileEndian;
use crate::{Error, HeaderValidationError, Mode, Reader};
use std::path::Path;

// ============================================================================
// Issue types
// ============================================================================

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single validation issue found during file inspection.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub category: &'static str,
    pub message: String,
}

impl ValidationIssue {
    fn error(category: &'static str, message: String) -> Self {
        Self { severity: Severity::Error, category, message }
    }
    fn warning(category: &'static str, message: String) -> Self {
        Self { severity: Severity::Warning, category, message }
    }
    fn info(category: &'static str, message: String) -> Self {
        Self { severity: Severity::Info, category, message }
    }
}

// ============================================================================
// ValidationReport
// ============================================================================

/// Structured result of a full MRC file validation.
///
/// Returned by [`validate_full`].  The file passes validation when
/// no [`Severity::Error`] issues are present.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Path to the validated file.
    pub path: String,
    /// Detected compression format.
    pub compression: String,
    /// Volume dimensions.
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
    /// MRC data mode.
    pub mode: i32,
    /// All issues discovered (errors + warnings + info).
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// `true` when no error-severity issues were found.
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// All issues at a given severity level.
    pub fn by_severity(&self, severity: Severity) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(move |i| i.severity == severity)
    }
}

// ============================================================================
// Full validation
// ============================================================================

/// Run comprehensive validation on an MRC file.
///
/// Opens the file, checks header structure, data statistics, and data
/// integrity.  In permissive mode, non-critical header issues are reported
/// as warnings rather than hard errors.
///
/// # Errors
/// Returns `Err` only when the file cannot be opened or read at all.
pub fn validate_full<P: AsRef<Path>>(
    path: P,
    permissive: bool,
) -> Result<ValidationReport, Error> {
    let path_str = path.as_ref().to_string_lossy().into_owned();

    // ── Detect compression ──
    let compression = match crate::io::reader::detect_compression(&path)? {
        crate::io::reader::CompressionType::Plain => "plain".into(),
        #[cfg(feature = "gzip")]
        crate::io::reader::CompressionType::Gzip => "gzip".into(),
        #[cfg(feature = "bzip2")]
        crate::io::reader::CompressionType::Bzip2 => "bzip2".into(),
    };

    // ── Open the file ──
    let (reader, open_warnings) = if permissive {
        Reader::open_permissive(&path)?
    } else {
        (Reader::open(&path)?, Vec::new())
    };

    let mut issues: Vec<ValidationIssue> = Vec::new();
    let header = reader.header();
    let mode_val = header.mode;
    let endian = reader.endian();

    // ── Open warnings (permissive mode) ──
    for w in &open_warnings {
        issues.push(ValidationIssue::warning("Open", w.clone()));
    }

    // ── 1. Header structure ──
    match header.validate_detailed() {
        Ok(()) => {
            issues.push(ValidationIssue::info("Header", "Structure is valid".into()));
        }
        Err(e) => {
            let desc = match &e {
                HeaderValidationError::InvalidDimensions { nx, ny, nz } =>
                    format!("Dimensions ({nx}×{ny}×{nz}) must all be positive"),
                HeaderValidationError::UnsupportedMode(m) =>
                    format!("Unsupported mode value: {m}"),
                HeaderValidationError::InvalidMap(m) =>
                    format!("MAP field is {:?}, expected b\"MAP \"", std::str::from_utf8(m).unwrap_or("?")),
                HeaderValidationError::InvalidIspg(s) =>
                    format!("ISPG {s} is outside valid ranges (0, 1–230, 400–630)"),
                HeaderValidationError::InvalidAxisMapping { mapc, mapr, maps } =>
                    format!("Axis mapping ({mapc}, {mapr}, {maps}) is not a permutation of 1,2,3"),
                HeaderValidationError::InvalidNsymbt(s) =>
                    format!("NSYMBT is negative ({s})"),
                HeaderValidationError::InvalidNlabl(n) =>
                    format!("NLABL is {n}, must be 0–10"),
                HeaderValidationError::InvalidNversion(n) =>
                    format!("NVERSION is {n}, expected 20140 or 20141"),
                HeaderValidationError::InvalidSampling { mx, my, mz } =>
                    format!("Sampling ({mx}×{my}×{mz}) must all be positive"),
                HeaderValidationError::InvalidVolumeStack { nz, mz, ispg } =>
                    format!("Volume stack: nz={nz} not divisible by mz={mz} for ispg={ispg}"),
                HeaderValidationError::LabelCountMismatch { nlabl, actual } =>
                    format!("nlabl={nlabl} but {actual} non-empty labels found"),
                HeaderValidationError::EmptyLabelBeforeFilled { index } =>
                    format!("Empty label at index {index} before all filled labels"),
            };
            issues.push(ValidationIssue::error("Header", desc));
        }
    }

    // ── 2. File size ──
    if let Some(data_size) = header.data_size() {
        let expected_total = 1024 + header.nsymbt.max(0) as usize + data_size;
        issues.push(ValidationIssue::info("File size",
            format!("Expected {} bytes (header + ext + data)", expected_total)));
    }

    // ── 3. Endianness ──
    let machst_info = FileEndian::from_machst_with_info(&header.machst);
    if !machst_info.is_standard {
        issues.push(ValidationIssue::warning("Endianness",
            format!("Non-standard MACHST stamp: {}", machst_info.description)));
    }
    let host = FileEndian::native();
    if endian != host {
        issues.push(ValidationIssue::info("Endianness",
            format!("Non-native byte order ({:?}), host is {:?}", endian, host)));
    } else {
        issues.push(ValidationIssue::info("Endianness",
            "Native byte order, fast-path available".into()));
    }

    // ── 4. Data statistics ──
    let data_bytes = reader.data_bytes();
    match compute_stats(data_bytes, reader.mode(), endian) {
        Ok((actual_dmin, actual_dmax, actual_dmean, actual_rms)) => {
            let complex = matches!(reader.mode(),
                Mode::Float32Complex | Mode::Int16Complex);

            let stats_unset = header.dmin > header.dmax;
            let rms_unset = header.rms < 0.0;

            let rtol = 0.01f32;

            let min_ok = complex || stats_unset
                || crate::engine::stats::is_close(header.dmin, actual_dmin, rtol);
            let max_ok = complex || stats_unset
                || crate::engine::stats::is_close(header.dmax, actual_dmax, rtol);
            let mean_ok = complex || stats_unset
                || crate::engine::stats::is_close(header.dmean, actual_dmean, rtol);
            let rms_ok = rms_unset
                || crate::engine::stats::is_close(header.rms, actual_rms, rtol);

            if !stats_unset || !rms_unset {
                let mismatch_parts = Vec::new();
                let mut mismatch_parts = mismatch_parts;
                if !min_ok { mismatch_parts.push("dmin".to_string()); }
                if !max_ok { mismatch_parts.push("dmax".to_string()); }
                if !mean_ok { mismatch_parts.push("dmean".to_string()); }
                if !rms_ok { mismatch_parts.push("rms".to_string()); }

                if mismatch_parts.is_empty() {
                    if stats_unset {
                        issues.push(ValidationIssue::info("Statistics",
                            "Statistics not written in header (sentinel values)".into()));
                    } else {
                        issues.push(ValidationIssue::info("Statistics",
                            "All statistics match actual data (within 1 % tolerance)".into()));
                    }
                } else {
                    let mut detail = String::new();
                    if !min_ok {
                        detail.push_str(&format!(
                            " dmin claimed={} actual={}", header.dmin, actual_dmin));
                    }
                    if !max_ok {
                        detail.push_str(&format!(
                            " dmax claimed={} actual={}", header.dmax, actual_dmax));
                    }
                    if !mean_ok {
                        detail.push_str(&format!(
                            " dmean claimed={} actual={}", header.dmean, actual_dmean));
                    }
                    if !rms_ok {
                        detail.push_str(&format!(
                            " rms claimed={} actual={}", header.rms, actual_rms));
                    }
                    issues.push(ValidationIssue::error("Statistics",
                        format!("Mismatch:{}", detail)));
                }
            }
        }
        Err(e) => {
            issues.push(ValidationIssue::error("Statistics",
                format!("Cannot compute statistics: {e}")));
        }
    }

    // ── 5. Data integrity (scan for NaN / Inf in float modes) ──
    if reader.mode().is_float() && !reader.mode().is_complex() {
        match float_mode_issues(data_bytes, reader.mode(), endian) {
            Ok(has_issues) => {
                if has_issues.is_empty() {
                    issues.push(ValidationIssue::info("Data integrity",
                        "All voxel values are finite numbers".into()));
                } else {
                    for issue in has_issues {
                        issues.push(ValidationIssue::warning("Data integrity", issue));
                    }
                }
            }
            Err(e) => {
                issues.push(ValidationIssue::warning("Data integrity",
                    format!("Could not scan data: {e}")));
            }
        }
    }

    // ── 6. Volume info ──
    let vol_type = if header.is_single_image() {
        "single 2D image"
    } else if header.is_image_stack() {
        "image stack"
    } else if header.is_volume_stack() {
        let nvol = if header.mz > 0 { header.nz / header.mz } else { 0 };
        issues.push(ValidationIssue::info("Volume",
            format!("Volume stack: {nvol} sub-volumes × {} slices", header.mz)));
        "volume stack"
    } else {
        "3D volume"
    };
    issues.push(ValidationIssue::info("Volume",
        format!("{} × {} × {} voxels, {}",
            header.nx, header.ny, header.nz, vol_type)));

    Ok(ValidationReport {
        path: path_str,
        compression,
        nx: header.nx,
        ny: header.ny,
        nz: header.nz,
        mode: mode_val,
        issues,
    })
}

// ── Float-mode data integrity helper ──

fn float_mode_issues(
    data_bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
) -> Result<Vec<String>, Error> {
    use crate::engine::codec::decode_slice;
    let data: Vec<f32> = match mode {
        Mode::Float32 => decode_slice::<f32>(data_bytes, endian)?,
        Mode::Float16 => {
            #[cfg(feature = "f16")]
            {
                let f16_data = decode_slice::<crate::f16>(data_bytes, endian)?;
                f16_data.iter().map(|&v| f32::from(v)).collect()
            }
            #[cfg(not(feature = "f16"))]
            return Ok(Vec::new());
        }
        _ => return Ok(Vec::new()),
    };

    let mut issues = Vec::new();
    let mut nan_count = 0usize;
    let mut inf_count = 0usize;
    let mut neg_inf_count = 0usize;

    for &v in &data {
        if v.is_nan() {
            nan_count += 1;
        } else if v.is_infinite() {
            if v.is_sign_negative() {
                neg_inf_count += 1;
            } else {
                inf_count += 1;
            }
        }
    }

    if nan_count > 0 {
        issues.push(format!("{nan_count} NaN values found ({:.2}%)",
            nan_count as f64 / data.len() as f64 * 100.0));
    }
    if inf_count > 0 {
        issues.push(format!("{inf_count} +Inf values found ({:.2}%)",
            inf_count as f64 / data.len() as f64 * 100.0));
    }
    if neg_inf_count > 0 {
        issues.push(format!("{neg_inf_count} -Inf values found ({:.2}%)",
            neg_inf_count as f64 / data.len() as f64 * 100.0));
    }
    Ok(issues)
}
