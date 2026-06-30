//! MRC file validation CLI tool.
//!
//! Usage:
//! ```text
//! mrc-validate <file.mrc>
//! mrc-validate --permissive <file.mrc>
//! mrc-validate --stats-only <file.mrc>
//! ```

use mrc::{CompressionType, Error, Reader, detect_compression};
use std::env;
use std::process;

fn usage() {
    eprintln!("mrc-validate v{} — MRC file validation tool", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Validates an MRC file by checking header structure and cross-referencing");
    eprintln!("data statistics (dmin, dmax, dmean, rms) against actual voxel values.");
    eprintln!("Auto-detects gzip/bzip2 compression.");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  mrc-validate [OPTIONS] <file>");
    eprintln!();
    eprintln!("ARGS:");
    eprintln!("  <file>         Path to an MRC file (.mrc, .mrc.gz, .mrc.bz2)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  -p, --permissive   Report non-critical header issues as warnings");
    eprintln!("                     instead of hard errors");
    eprintln!("  -s, --stats-only   Skip header validation, only cross-check statistics");
    eprintln!("  -h, --help         Print this help message");
    eprintln!();
    eprintln!("EXIT CODES:");
    eprintln!("  0    File is valid (or warnings only in permissive mode)");
    eprintln!("  1    Validation failed (header invalid or stats mismatch)");
    eprintln!("  2    Usage error or file could not be opened");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  mrc-validate protein.mrc");
    eprintln!("  mrc-validate --permissive legacy.mrc");
    eprintln!("  mrc-validate --stats-only large_volume.mrc");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        process::exit(0);
    }

    let mut permissive = false;
    let mut stats_only = false;
    let mut path: Option<&str> = None;

    for arg in &args {
        match arg.as_str() {
            "--permissive" | "-p" => permissive = true,
            "--stats-only" | "-s" => stats_only = true,
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                usage();
                process::exit(2);
            }
            _ => {
                if path.is_some() {
                    eprintln!("Only one file can be validated at a time");
                    usage();
                    process::exit(2);
                }
                path = Some(arg);
            }
        }
    }

    let path = match path {
        Some(p) => p,
        None => {
            eprintln!("No file specified");
            usage();
            process::exit(2);
        }
    };

    // Detect compression for informational output
    let compression = match detect_compression(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error opening '{}': {}", path, e);
            process::exit(2);
        }
    };

    let compression_label = match compression {
        CompressionType::Plain => "plain",
        #[cfg(feature = "gzip")]
        CompressionType::Gzip => "gzip",
        #[cfg(feature = "bzip2")]
        CompressionType::Bzip2 => "bzip2",
    };

    // Open the file
    let (reader, warnings) = if permissive {
        match Reader::open_permissive(path) {
            Ok(rw) => rw,
            Err(e) => {
                print_result(path, compression_label, &e, &[], false);
                process::exit(1);
            }
        }
    } else {
        match Reader::open(path) {
            Ok(r) => (r, Vec::new()),
            Err(e) => {
                print_result(path, compression_label, &e, &[], false);
                process::exit(1);
            }
        }
    };

    let header = reader.header();
    let shape = reader.shape();
    let mode = reader.mode();

    // Header validation (unless --stats-only)
    let header_ok = if stats_only {
        true
    } else {
        match header.validate_detailed() {
            Ok(()) => true,
            Err(e) => {
                print_result(
                    path,
                    compression_label,
                    &Error::InvalidHeaderDetailed(e),
                    &warnings,
                    false,
                );
                process::exit(1);
            }
        }
    };

    // Stats cross-check
    let stats_ok = match reader.validate_header_stats() {
        Ok(()) => true,
        Err(Error::StatsMismatch {
            claimed_dmin,
            claimed_dmax,
            claimed_dmean,
            claimed_rms,
            actual_dmin,
            actual_dmax,
            actual_dmean,
            actual_rms,
        }) => {
            print_result(
                path,
                compression_label,
                &Error::StatsMismatch {
                    claimed_dmin,
                    claimed_dmax,
                    claimed_dmean,
                    claimed_rms,
                    actual_dmin,
                    actual_dmax,
                    actual_dmean,
                    actual_rms,
                },
                &warnings,
                false,
            );
            process::exit(1);
        }
        Err(e) => {
            print_result(path, compression_label, &e, &warnings, false);
            process::exit(1);
        }
    };

    print_result(
        path,
        compression_label,
        &Error::InvalidHeader,
        &warnings,
        header_ok && stats_ok,
    );

    // Print file info
    println!();
    println!("  Dimensions: {} x {} x {}", shape.nx, shape.ny, shape.nz);
    println!("  Mode:       {:?}", mode);
    println!("  Endian:     {:?}", reader.endian());
    println!("  Voxel size: {:?}", header.voxel_size());
    println!("  Labels:     {}", header.get_labels().len());
    for (i, label) in header.get_labels().iter().enumerate() {
        println!("    [{}] {}", i, label);
    }

    if !warnings.is_empty() {
        println!();
        println!("  Warnings:");
        for w in &warnings {
            println!("    - {}", w);
        }
    }

    process::exit(0);
}

fn print_result(path: &str, compression: &str, error: &Error, _warnings: &[String], ok: bool) {
    if ok {
        println!("✅ {} ({}): VALID", path, compression);
    } else {
        match error {
            Error::InvalidHeaderDetailed(e) => {
                println!("❌ {} ({}): INVALID - {}", path, compression, e);
            }
            Error::StatsMismatch {
                claimed_dmin,
                claimed_dmax,
                claimed_dmean,
                claimed_rms,
                actual_dmin,
                actual_dmax,
                actual_dmean,
                actual_rms,
            } => {
                println!("❌ {} ({}): STATS MISMATCH", path, compression);
                println!(
                    "     Claimed: dmin={}, dmax={}, dmean={}, rms={}",
                    claimed_dmin, claimed_dmax, claimed_dmean, claimed_rms
                );
                println!(
                    "     Actual:  dmin={}, dmax={}, dmean={}, rms={}",
                    actual_dmin, actual_dmax, actual_dmean, actual_rms
                );
            }
            e => {
                println!("❌ {} ({}): ERROR - {}", path, compression, e);
            }
        }
    }
}
