//! MRC file validation infrastructure.
//!
//! Provides [`validate_full`] for comprehensive file validation,
//! [`validate_reader`] for validating an already-open reader, and
//! [`ValidationReport`] for structured results with categorized issues.
//!
//! # Quick check
//!
//! ```no_run
//! use mrc::validate::{validate_full, Severity};
//!
//! let report = validate_full("protein.mrc", false).unwrap();
//! if report.is_valid() {
//!     println!("File is valid");
//! } else {
//!     for issue in &report.issues {
//!         if issue.severity == Severity::Error {
//!             eprintln!("ERROR [{}]: {}", issue.category, issue.message);
//!         }
//!     }
//! }
//! ```

use crate::engine::endian::FileEndian;
use crate::engine::stats::compute_stats;
use crate::{Error, HeaderValidationError, Mode, Reader};
use std::path::Path;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ============================================================================
// Issue types
// ============================================================================

/// Severity of a validation issue.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The file cannot be used as-is (corrupt header, data mismatch, etc.).
    Error,
    /// The file is usable but has non-standard or suspicious properties.
    Warning,
    /// Informational message about the file contents.
    Info,
}

/// A single validation issue found during file inspection.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// How serious the issue is.
    pub severity: Severity,
    /// Short category label, e.g. `"Header"`, `"Statistics"`, `"Endianness"`.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub category: &'static str,
    /// Human-readable description of the issue.
    pub message: String,
}

impl ValidationIssue {
    fn error(category: &'static str, message: String) -> Self {
        Self {
            severity: Severity::Error,
            category,
            message,
        }
    }
    fn warning(category: &'static str, message: String) -> Self {
        Self {
            severity: Severity::Warning,
            category,
            message,
        }
    }
    fn info(category: &'static str, message: String) -> Self {
        Self {
            severity: Severity::Info,
            category,
            message,
        }
    }
}

// ============================================================================
// ValidationReport
// ============================================================================

/// Structured result of a full MRC file validation.
///
/// Returned by [`validate_full`].  The file passes validation when
/// no [`Severity::Error`] issues are present.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Path to the validated file.
    pub path: String,
    /// Detected compression format.
    pub compression: String,
    /// Number of columns.
    pub nx: i32,
    /// Number of rows.
    pub ny: i32,
    /// Number of sections.
    pub nz: i32,
    /// MRC data mode value.
    pub mode: i32,
    /// All issues discovered during validation.
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
// Validation implementations
// ============================================================================

/// Run comprehensive validation on an already-opened [`Reader`].
///
/// Checks header structure, file size, endianness, data statistics cross-check
/// (1% tolerance), and NaN/Inf scanning. Avoids the redundant open that
/// [`validate_full`] performs.
///
/// The `warnings` parameter accepts the permissive-mode warnings from
/// [`Reader::open_permissive`]; pass an empty slice for strict-mode opens.
///
/// # Errors
/// Returns `Err` only when reading or computing statistics fails.
pub fn validate_reader(
    reader: &Reader,
    path: &str,
    compression: &str,
    warnings: &[String],
) -> Result<ValidationReport, Error> {
    let mut issues: Vec<ValidationIssue> = Vec::new();
    let header = reader.header();
    let mode_val = header.mode;
    let endian = reader.endian();

    // ── Open warnings (permissive mode) ──
    for w in warnings {
        issues.push(ValidationIssue::warning("Open", w.clone()));
    }

    // ── 1. Header structure ──
    match header.validate_detailed() {
        Ok(()) => {
            issues.push(ValidationIssue::info("Header", "Structure is valid".into()));
        }
        Err(e) => {
            let desc = match &e {
                HeaderValidationError::InvalidDimensions { nx, ny, nz } => {
                    format!("Dimensions ({nx}×{ny}×{nz}) must all be positive")
                }
                HeaderValidationError::UnsupportedMode(m) => format!("Unsupported mode value: {m}"),
                HeaderValidationError::InvalidMap(m) => format!(
                    "MAP field is {:?}, expected b\"MAP \"",
                    std::str::from_utf8(m).unwrap_or("?")
                ),
                HeaderValidationError::InvalidIspg(s) => {
                    format!("ISPG {s} is outside valid ranges (0, 1–230, 400–630)")
                }
                HeaderValidationError::InvalidAxisMapping { mapc, mapr, maps } => {
                    format!("Axis mapping ({mapc}, {mapr}, {maps}) is not a permutation of 1,2,3")
                }
                HeaderValidationError::InvalidNsymbt(s) => format!("NSYMBT is negative ({s})"),
                HeaderValidationError::InvalidNlabl(n) => format!("NLABL is {n}, must be 0–10"),
                HeaderValidationError::InvalidNversion(n) => {
                    format!("NVERSION is {n}, expected 0, 20140, or 20141")
                }
                HeaderValidationError::InvalidSampling { mx, my, mz } => {
                    format!("Sampling ({mx}×{my}×{mz}) must all be positive")
                }
                HeaderValidationError::InvalidVolumeStack { nz, mz, ispg } => {
                    format!("Volume stack: nz={nz} not divisible by mz={mz} for ispg={ispg}")
                }
                HeaderValidationError::LabelCountMismatch { nlabl, actual } => {
                    format!("nlabl={nlabl} but {actual} non-empty labels found")
                }
                HeaderValidationError::EmptyLabelBeforeFilled { index } => {
                    format!("Empty label at index {index} before all filled labels")
                }
            };
            issues.push(ValidationIssue::error("Header", desc));
        }
    }

    // ── 2. File size ──
    if let Some(data_size) = header.data_size() {
        let expected_total = 1024 + header.nsymbt.max(0) as usize + data_size;
        issues.push(ValidationIssue::info(
            "File size",
            format!("Expected {} bytes (header + ext + data)", expected_total),
        ));
    }

    // ── 3. Endianness ──
    let machst_info = FileEndian::from_machst_with_info(&header.machst);
    if !machst_info.is_standard {
        issues.push(ValidationIssue::warning(
            "Endianness",
            format!("Non-standard MACHST stamp: {}", machst_info.description),
        ));
    }
    let host = FileEndian::native();
    if endian != host {
        issues.push(ValidationIssue::info(
            "Endianness",
            format!("Non-native byte order ({:?}), host is {:?}", endian, host),
        ));
    } else {
        issues.push(ValidationIssue::info(
            "Endianness",
            "Native byte order, fast-path available".into(),
        ));
    }

    // ── 4. Data statistics ──
    let data_bytes = reader.data_bytes();
    match compute_stats(
        data_bytes,
        reader.mode(),
        endian,
        reader.shape().nx,
        reader.shape().ny * reader.shape().nz,
    ) {
        Ok((actual_dmin, actual_dmax, actual_dmean, actual_rms)) => {
            let complex = matches!(reader.mode(), Mode::Float32Complex | Mode::Int16Complex);

            let stats_unset = header.dmin > header.dmax;
            let rms_unset = header.rms < 0.0;

            let rtol = 0.01f32;

            let min_ok = complex
                || stats_unset
                || crate::engine::stats::is_close(header.dmin, actual_dmin, rtol);
            let max_ok = complex
                || stats_unset
                || crate::engine::stats::is_close(header.dmax, actual_dmax, rtol);
            let mean_ok = complex
                || stats_unset
                || crate::engine::stats::is_close(header.dmean, actual_dmean, rtol);
            let rms_ok = rms_unset || crate::engine::stats::is_close(header.rms, actual_rms, rtol);

            if !stats_unset || !rms_unset {
                let mut mismatch_parts = Vec::new();
                if !min_ok {
                    mismatch_parts.push("dmin".to_string());
                }
                if !max_ok {
                    mismatch_parts.push("dmax".to_string());
                }
                if !mean_ok {
                    mismatch_parts.push("dmean".to_string());
                }
                if !rms_ok {
                    mismatch_parts.push("rms".to_string());
                }

                if mismatch_parts.is_empty() {
                    if stats_unset {
                        issues.push(ValidationIssue::info(
                            "Statistics",
                            "Statistics not written in header (sentinel values)".into(),
                        ));
                    } else {
                        issues.push(ValidationIssue::info(
                            "Statistics",
                            "All statistics match actual data (within 1% tolerance)".into(),
                        ));
                    }
                } else {
                    let mut detail = String::new();
                    if !min_ok {
                        detail.push_str(&format!(
                            " dmin claimed={} actual={}",
                            header.dmin, actual_dmin
                        ));
                    }
                    if !max_ok {
                        detail.push_str(&format!(
                            " dmax claimed={} actual={}",
                            header.dmax, actual_dmax
                        ));
                    }
                    if !mean_ok {
                        detail.push_str(&format!(
                            " dmean claimed={} actual={}",
                            header.dmean, actual_dmean
                        ));
                    }
                    if !rms_ok {
                        detail.push_str(&format!(
                            " rms claimed={} actual={}",
                            header.rms, actual_rms
                        ));
                    }
                    issues.push(ValidationIssue::error(
                        "Statistics",
                        format!("Mismatch:{}", detail),
                    ));
                }
            }
        }
        Err(e) => {
            issues.push(ValidationIssue::error(
                "Statistics",
                format!("Cannot compute statistics: {e}"),
            ));
        }
    }

    // ── 5. Data integrity (scan for NaN / Inf in float modes) ──
    if reader.mode().is_float() && !reader.mode().is_complex() {
        match float_mode_issues(data_bytes, reader.mode(), endian) {
            Ok(has_issues) => {
                if has_issues.is_empty() {
                    issues.push(ValidationIssue::info(
                        "Data integrity",
                        "All voxel values are finite numbers".into(),
                    ));
                } else {
                    for issue in has_issues {
                        issues.push(ValidationIssue::warning("Data integrity", issue));
                    }
                }
            }
            Err(e) => {
                issues.push(ValidationIssue::warning(
                    "Data integrity",
                    format!("Could not scan data: {e}"),
                ));
            }
        }
    } else if reader.mode() == Mode::Float32Complex {
        match complex_float_mode_issues(data_bytes, endian) {
            Ok(has_issues) => {
                if has_issues.is_empty() {
                    issues.push(ValidationIssue::info(
                        "Data integrity",
                        "All complex values are finite numbers".into(),
                    ));
                } else {
                    for issue in has_issues {
                        issues.push(ValidationIssue::warning("Data integrity", issue));
                    }
                }
            }
            Err(e) => {
                issues.push(ValidationIssue::warning(
                    "Data integrity",
                    format!("Could not scan complex data: {e}"),
                ));
            }
        }
    }

    // ── 6. Volume info ──
    let vol_type = if header.is_single_image() {
        "single 2D image"
    } else if header.is_image_stack() {
        "image stack"
    } else if header.is_volume_stack() {
        let nvol = if header.mz > 0 {
            header.nz / header.mz
        } else {
            0
        };
        issues.push(ValidationIssue::info(
            "Volume",
            format!("Volume stack: {nvol} sub-volumes × {} slices", header.mz),
        ));
        "volume stack"
    } else {
        "3D volume"
    };
    issues.push(ValidationIssue::info(
        "Volume",
        format!(
            "{} × {} × {} voxels, {}",
            header.nx, header.ny, header.nz, vol_type
        ),
    ));

    Ok(ValidationReport {
        path: path.to_owned(),
        compression: compression.to_owned(),
        nx: header.nx,
        ny: header.ny,
        nz: header.nz,
        mode: mode_val,
        issues,
    })
}

/// Run comprehensive validation on an MRC file.
///
/// Opens the file, checks header structure, data statistics, and data
/// integrity. In permissive mode, non-critical header issues are reported
/// as warnings rather than hard errors.
///
/// Prefer [`validate_reader`] when you already have an open [`Reader`] — it
/// avoids the redundant file I/O.
///
/// # Errors
/// Returns `Err` only when the file cannot be opened or read at all.
pub fn validate_full<P: AsRef<Path>>(path: P, permissive: bool) -> Result<ValidationReport, Error> {
    let path_str = path.as_ref().to_string_lossy().into_owned();

    let compression = match crate::io::reader::detect_compression(&path)? {
        crate::io::reader::CompressionType::Plain => "plain".to_string(),
        #[cfg(feature = "gzip")]
        crate::io::reader::CompressionType::Gzip => "gzip".to_string(),
        #[cfg(feature = "bzip2")]
        crate::io::reader::CompressionType::Bzip2 => "bzip2".to_string(),
    };

    let (reader, warnings) = if permissive {
        Reader::open_permissive(&path)?
    } else {
        (Reader::open(&path)?, Vec::new())
    };

    validate_reader(&reader, &path_str, &compression, &warnings)
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
            return Ok(vec![
                "Float16 scanning unavailable (requires `f16` feature)".into(),
            ]);
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
        issues.push(format!(
            "{nan_count} NaN values found ({:.2}%)",
            nan_count as f64 / data.len() as f64 * 100.0
        ));
    }
    if inf_count > 0 {
        issues.push(format!(
            "{inf_count} +Inf values found ({:.2}%)",
            inf_count as f64 / data.len() as f64 * 100.0
        ));
    }
    if neg_inf_count > 0 {
        issues.push(format!(
            "{neg_inf_count} -Inf values found ({:.2}%)",
            neg_inf_count as f64 / data.len() as f64 * 100.0
        ));
    }
    Ok(issues)
}

/// Scan Float32Complex data for NaN/Inf values in both real and imaginary parts.
fn complex_float_mode_issues(data_bytes: &[u8], endian: FileEndian) -> Result<Vec<String>, Error> {
    use crate::engine::codec::decode_slice;
    let data: Vec<crate::mode::Float32Complex> =
        decode_slice::<crate::mode::Float32Complex>(data_bytes, endian)?;

    let mut issues = Vec::new();
    let mut nan_count = 0usize;
    let mut inf_count = 0usize;
    let mut neg_inf_count = 0usize;

    for c in &data {
        for &v in &[c.real, c.imag] {
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
    }

    let total_components = data.len() * 2;
    if nan_count > 0 {
        issues.push(format!(
            "{nan_count} NaN values found in complex components ({:.2}%)",
            nan_count as f64 / total_components as f64 * 100.0
        ));
    }
    if inf_count > 0 {
        issues.push(format!(
            "{inf_count} +Inf values found in complex components ({:.2}%)",
            inf_count as f64 / total_components as f64 * 100.0
        ));
    }
    if neg_inf_count > 0 {
        issues.push(format!(
            "{neg_inf_count} -Inf values found in complex components ({:.2}%)",
            neg_inf_count as f64 / total_components as f64 * 100.0
        ));
    }
    Ok(issues)
}
