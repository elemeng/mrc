//! MRC validation CLI using the full validation report.
//!
//! Usage:
//! ```text
//! mrc-validate [--permissive] <file.mrc>
//! ```

use mrc::validate::{Severity, ValidationIssue, validate_full};
use std::env;
use std::process;

fn usage() {
    eprintln!("mrc-validate v{} — MRC file validation tool", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Performs comprehensive validation on an MRC file:");
    eprintln!("  • Header structure — dimensions, mode, MAP, axis mapping, labels");
    eprintln!("  • File size consistency");
    eprintln!("  • Endianness detection");
    eprintln!("  • Data statistics — cross-checks dmin/dmax/dmean/rms (1 % tolerance)");
    eprintln!("  • Data integrity — scans for NaN / Inf values in float modes");
    eprintln!("  • Volume type classification");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  mrc-validate [OPTIONS] <file>");
    eprintln!();
    eprintln!("ARGS:");
    eprintln!("  <file>         Path to an MRC file (.mrc, .mrc.gz, .mrc.bz2)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  -p, --permissive   Report non-critical header issues as warnings");
    eprintln!("  -h, --help         Print this help message");
    eprintln!();
    eprintln!("EXIT CODES:");
    eprintln!("  0    File is valid");
    eprintln!("  1    Validation failed (errors found)");
    eprintln!("  2    Usage error or file could not be opened");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  mrc-validate protein.mrc");
    eprintln!("  mrc-validate --permissive legacy.mrc");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        process::exit(0);
    }

    let mut permissive = false;
    let mut path: Option<&str> = None;

    for arg in &args {
        match arg.as_str() {
            "--permissive" | "-p" => permissive = true,
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

    let report = match validate_full(path, permissive) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error validating '{}': {}", path, e);
            process::exit(2);
        }
    };

    // ── Print report ──
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              MRC Validation Report                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let status = if report.is_valid() { "✅ VALID" } else { "❌ INVALID" };
    println!("  File:     {}", report.path);
    println!("  Format:   {} ({} compression)", report.compression,
        if report.compression == "plain" { "none" } else { &report.compression });
    println!("  Status:   {}", status);
    println!();

    // Group issues by category
    let mut categories: Vec<&str> = Vec::new();
    let mut cat_map: std::collections::BTreeMap<&str, Vec<&ValidationIssue>> =
        std::collections::BTreeMap::new();
    for issue in &report.issues {
        cat_map.entry(issue.category).or_default().push(issue);
        if !categories.contains(&issue.category) {
            categories.push(issue.category);
        }
    }

    for cat in categories {
        let items = cat_map.get(cat).unwrap();
        let error_count = items.iter().filter(|i| i.severity == Severity::Error).count();
        let warn_count = items.iter().filter(|i| i.severity == Severity::Warning).count();

        let summary = match (error_count, warn_count) {
            (0, 0) => format!("── {} ──", cat),
            (e, w) if e > 0 && w > 0 => format!("── {} ── {} error(s), {} warning(s)", cat, e, w),
            (e, 0) if e > 0 => format!("── {} ── {} error(s)", cat, e),
            (0, w) => format!("── {} ── {} warning(s)", cat, w),
            _ => unreachable!(),
        };
        println!("{}", summary);

        for issue in items {
            let icon = match issue.severity {
                Severity::Error   => "  ❌ ",
                Severity::Warning => "  ⚠️  ",
                Severity::Info    => "  ℹ️  ",
            };
            println!("{}{}", icon, issue.message);
        }
        println!();
    }

    process::exit(if report.is_valid() { 0 } else { 1 });
}
