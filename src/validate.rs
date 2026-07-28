//! MRC file validation infrastructure.
//!
//! Provides [`validate_full`] for comprehensive file validation,
//! [`validate_reader`] for validating an already-open reader, and
//! [`ValidationReport`] for structured results with categorized issues.
//!
//! # Quick check
//!
//! ```no_run
//! use mrc::{validate_full, Severity};
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
use crate::engine::stats::validate_header_stats;
use crate::{Error, Mode, Reader};
use std::path::Path;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ============================================================================
// Issue types
// ============================================================================

/// Severity of a validation issue.
///
/// # Example
///
/// ```rust
/// use mrc::Severity;
///
/// let s = Severity::Error;
/// assert!(matches!(s, Severity::Error));
/// ```
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
///
/// # Example
///
/// ```rust
/// use mrc::{Severity, ValidationIssue};
///
/// let issue = ValidationIssue {
///     severity: Severity::Warning,
///     category: "Header",
///     message: "Non-standard MACHST stamp".into(),
/// };
/// assert_eq!(issue.severity, Severity::Warning);
/// ```
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
///
/// # Example
///
/// ```rust
/// use mrc::{ValidationReport, Severity, ValidationIssue};
///
/// let report = ValidationReport {
///     path: "test.mrc".into(),
///     compression: "plain".into(),
///     nx: 10, ny: 10, nz: 10,
///     mode: 2,
///     issues: vec![],
/// };
/// assert!(report.is_valid());
/// ```
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
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::{ValidationReport, Severity, ValidationIssue};
    ///
    /// let report = ValidationReport {
    ///     path: "test.mrc".into(),
    ///     compression: "plain".into(),
    ///     nx: 10, ny: 10, nz: 10, mode: 2,
    ///     issues: vec![ValidationIssue {
    ///         severity: Severity::Error,
    ///         category: "Header",
    ///         message: "bad header".into(),
    ///     }],
    /// };
    /// assert!(!report.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// All issues at a given severity level.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mrc::{ValidationReport, Severity, ValidationIssue};
    ///
    /// let report = ValidationReport {
    ///     path: "test.mrc".into(),
    ///     compression: "plain".into(),
    ///     nx: 10, ny: 10, nz: 10, mode: 2,
    ///     issues: vec![ValidationIssue {
    ///         severity: Severity::Warning,
    ///         category: "Endianness",
    ///         message: "Non-native byte order".into(),
    ///     }],
    /// };
    /// let warnings: Vec<_> = report.by_severity(Severity::Warning).collect();
    /// assert_eq!(warnings.len(), 1);
    /// ```
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
///
/// # Example
///
/// ```no_run
/// use mrc::Reader;
/// use mrc::validate_reader;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let reader = Reader::open("protein.mrc")?;
/// let report = validate_reader(&reader, "protein.mrc", "plain", &[])?;
/// println!("Is valid: {}", report.is_valid());
/// # Ok(())
/// # }
/// ```
pub fn validate_reader(
    reader: &Reader,
    path: &str,
    compression: &str,
    warnings: &[String],
) -> Result<ValidationReport, Error> {
    let mut issues: Vec<ValidationIssue> = Vec::with_capacity(16);
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
        Err(e) => issues.push(ValidationIssue::error("Header", e.to_string())),
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
    let raw_bytes = reader.raw_bytes();
    match check_statistics(header, raw_bytes) {
        Some(issue) => issues.push(issue),
        None => issues.push(ValidationIssue::info(
            "Statistics",
            "All statistics match actual data (within 1% tolerance)".into(),
        )),
    }

    // ── 5. Data integrity (scan for NaN / Inf in float modes) ──
    if reader.mode().is_float() && !reader.mode().is_complex() {
        match float_mode_issues(raw_bytes, reader.mode(), endian) {
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
        match complex_float_mode_issues(raw_bytes, endian) {
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
///
/// # Example
///
/// ```no_run
/// use mrc::validate_full;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let report = validate_full("protein.mrc", false)?;
/// if report.is_valid() {
///     println!("File is valid");
/// }
/// # Ok(())
/// # }
/// ```
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

/// Compare header statistics against computed values using
/// [`validate_header_stats`]. Returns `Some(ValidationIssue)` on mismatch or
/// error, `None` if all statistics match within 1% tolerance.
fn check_statistics(
    header: &crate::Header,
    raw_bytes: &[u8],
) -> Option<ValidationIssue> {
    match validate_header_stats(header, raw_bytes) {
        Ok(()) => None,
        Err(crate::Error::StatsMismatch { claimed_dmin, claimed_dmax, claimed_dmean, claimed_rms, actual_dmin, actual_dmax, actual_dmean, actual_rms }) => {
            let mut detail = String::new();
            if (claimed_dmin - actual_dmin).abs() > 0.01 * actual_dmin.abs().max(claimed_dmin.abs()) {
                detail.push_str(&format!(" dmin claimed={claimed_dmin} actual={actual_dmin}"));
            }
            if (claimed_dmax - actual_dmax).abs() > 0.01 * actual_dmax.abs().max(claimed_dmax.abs()) {
                detail.push_str(&format!(" dmax claimed={claimed_dmax} actual={actual_dmax}"));
            }
            if (claimed_dmean - actual_dmean).abs() > 0.01 * actual_dmean.abs().max(claimed_dmean.abs()) {
                detail.push_str(&format!(" dmean claimed={claimed_dmean} actual={actual_dmean}"));
            }
            if (claimed_rms - actual_rms).abs() > 0.01 * actual_rms.abs().max(claimed_rms.abs()) {
                detail.push_str(&format!(" rms claimed={claimed_rms} actual={actual_rms}"));
            }
            Some(ValidationIssue::error("Statistics", format!("Mismatch:{detail}")))
        }
        Err(e) => Some(ValidationIssue::error("Statistics", format!("Cannot compute statistics: {e}"))),
    }
}

// ── Float-mode data integrity helpers ──

struct NonFiniteCounts {
    nan: usize,
    pos_inf: usize,
    neg_inf: usize,
}

fn count_non_finite(values: impl Iterator<Item = f32>) -> NonFiniteCounts {
    let mut counts = NonFiniteCounts { nan: 0, pos_inf: 0, neg_inf: 0 };
    for v in values {
        if v.is_nan() {
            counts.nan += 1;
        } else if v.is_infinite() {
            if v.is_sign_negative() {
                counts.neg_inf += 1;
            } else {
                counts.pos_inf += 1;
            }
        }
    }
    counts
}

fn format_finite_issues(counts: &NonFiniteCounts, total: usize, prefix: &str) -> Vec<String> {
    let mut issues = Vec::with_capacity(3);
    if counts.nan > 0 {
        issues.push(format!(
            "{} NaN values found{} ({:.2}%)",
            counts.nan, prefix, counts.nan as f64 / total as f64 * 100.0
        ));
    }
    if counts.pos_inf > 0 {
        issues.push(format!(
            "{} +Inf values found{} ({:.2}%)",
            counts.pos_inf, prefix, counts.pos_inf as f64 / total as f64 * 100.0
        ));
    }
    if counts.neg_inf > 0 {
        issues.push(format!(
            "{} -Inf values found{} ({:.2}%)",
            counts.neg_inf, prefix, counts.neg_inf as f64 / total as f64 * 100.0
        ));
    }
    issues
}

fn float_mode_issues(
    raw_bytes: &[u8],
    mode: Mode,
    endian: FileEndian,
) -> Result<Vec<String>, Error> {
    use crate::engine::codec::decode_slice;
    use crate::engine::codec::EndianCodec;
    match mode {
        Mode::Float32 => {
            let data = decode_slice::<f32>(raw_bytes, endian)?;
            let counts = count_non_finite(data.iter().copied());
            Ok(format_finite_issues(&counts, data.len(), ""))
        }
        Mode::Float16 => {
            #[cfg(feature = "f16")]
            {
                // Fused: scan f16 bytes without allocating Vec<f32>.
                let n = raw_bytes.len() / 2;
                let mut counts = NonFiniteCounts { nan: 0, pos_inf: 0, neg_inf: 0 };
                for i in 0..n {
                    let v = f32::from(crate::f16::from_bytes(raw_bytes, i * 2, endian));
                    if v.is_nan() { counts.nan += 1; }
                    else if v.is_infinite() {
                        if v.is_sign_negative() { counts.neg_inf += 1; }
                        else { counts.pos_inf += 1; }
                    }
                }
                Ok(format_finite_issues(&counts, n, ""))
            }
            #[cfg(not(feature = "f16"))]
            Ok(vec!["Float16 scanning unavailable (requires `f16` feature)".into()])
        }
        _ => Ok(Vec::new()),
    }
}

/// Scan Float32Complex data for NaN/Inf values in both real and imaginary parts.
fn complex_float_mode_issues(raw_bytes: &[u8], endian: FileEndian) -> Result<Vec<String>, Error> {
    use crate::engine::codec::decode_slice;
    let data: Vec<crate::mode::Float32Complex> =
        decode_slice::<crate::mode::Float32Complex>(raw_bytes, endian)?;

    let counts = count_non_finite(data.iter().flat_map(|c| [c.real, c.imag]));
    Ok(format_finite_issues(&counts, data.len() * 2, " in complex components"))
}
