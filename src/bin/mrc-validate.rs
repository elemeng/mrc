//! MRC validation CLI using the full validation report.
//!
//! Usage:
//! ```text
//! mrc-validate [--permissive] <file.mrc>
//! mrc-validate [--permissive] --field <name> <file.mrc>
//! ```

use mrc::validate::{Severity, ValidationIssue, validate_full};
use std::env;
use std::process;

fn usage() {
    eprintln!("mrc-validate v{} — MRC file validation tool", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Performs comprehensive validation on an MRC file.");
    eprintln!("Auto-detects gzip/bzip2 compression.");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  mrc-validate [OPTIONS] <file>");
    eprintln!("  mrc-validate [OPTIONS] --field <name> <file>");
    eprintln!();
    eprintln!("ARGS:");
    eprintln!("  <file>                     Path to an MRC file");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  -p, --permissive           Report non-critical issues as warnings");
    eprintln!("  -f, --field <name>         Validate a single field (use --list-fields)");
    eprintln!("  -l, --list-fields          List available field names and aliases");
    eprintln!("  -h, --help                 Print this help message");
    eprintln!();
    eprintln!("EXIT CODES:");
    eprintln!("  0    File is valid");
    eprintln!("  1    Validation failed (errors found)");
    eprintln!("  2    Usage error or file could not be opened");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  mrc-validate protein.mrc");
    eprintln!("  mrc-validate --field mode protein.mrc");
    eprintln!("  mrc-validate --field stats --field dims protein.mrc");
    eprintln!("  mrc-validate --permissive legacy.mrc");
}

fn list_fields() {
    println!("Available --field values:");
    println!();
    let fields = [
        ("dims",       "Volume dimensions"),
        ("mode",       "Data mode value"),
        ("map",        "MAP identifier field"),
        ("ispg",       "Space group number"),
        ("axis",       "Axis mapping (MAPC/MAPR/MAPS)"),
        ("nsymbt",     "Extended header size"),
        ("nlabl",      "Label count"),
        ("nversion",   "MRC format version"),
        ("sampling",   "Cell sampling"),
        ("volume-stack","Volume stack (nz ÷ mz)"),
        ("labels",     "Label sequence"),
        ("stats",      "Data statistics cross-check"),
        ("endian",     "Endianness / MACHST"),
        ("integrity",  "NaN/Inf scan (float modes)"),
        ("all",        "All checks (default)"),
    ];
    for (name, desc) in &fields {
        println!("  {:<16} {}", name, desc);
    }
    println!();
    println!("Aliases: dims/dimensions, stats/statistics, endian/endianness/machst,");
    println!("         integrity/data, nsymbt/ext/extended, ispg/spacegroup, etc.");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        process::exit(0);
    }

    let mut permissive = false;
    let mut fields: Vec<String> = Vec::new();
    let mut path: Option<String> = None;
    let mut list = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--permissive" | "-p" => permissive = true,
            "--field" | "-f" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --field requires a value");
                    list_fields();
                    process::exit(2);
                }
                fields.push(args[i].clone());
            }
            "--list-fields" | "-l" => list = true,
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                usage();
                process::exit(2);
            }
            _ => {
                if path.is_some() {
                    eprintln!("Error: only one file can be validated at a time");
                    usage();
                    process::exit(2);
                }
                path = Some(arg.clone());
            }
        }
        i += 1;
    }

    if list {
        list_fields();
        process::exit(0);
    }

    let path = match path {
        Some(p) => p,
        None => {
            eprintln!("Error: no file specified");
            usage();
            process::exit(2);
        }
    };

    let report = match validate_full(&path, permissive) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error validating '{}': {}", path, e);
            process::exit(2);
        }
    };

    // Filter to requested fields (with name normalization)
    let mut issues: Vec<&ValidationIssue> = report.issues.iter().collect();
    if !fields.is_empty() && !fields.contains(&"all".to_string()) {
        let norm_fields: Vec<String> = fields.iter().map(|f| normalize_field(f)).collect();
        issues.retain(|i| {
            let cat_norm = i.category.to_lowercase().replace(' ', "-");
            norm_fields.iter().any(|f| f == &cat_norm || f == &i.category.to_lowercase())
        });
    }

    // ── Print report ──
    if fields.is_empty() || fields.contains(&"all".to_string()) {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║              MRC Validation Report                         ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        let status = if report.is_valid() { "✅ VALID" } else { "❌ INVALID" };
        println!("  File:     {}", report.path);
        println!("  Format:   {} ({})", report.compression,
            if report.compression == "plain" { "uncompressed" } else { &report.compression });
        println!("  Status:   {}", status);
        println!("  Mode:     {} ({})", report.mode,
            mrc::Mode::from_i32(report.mode).map(|m| format!("{m:?}")).unwrap_or("?".into()));
        println!("  Size:     {} × {} × {}", report.nx, report.ny, report.nz);
        println!();
    }

    // Group by category
    let mut cat_map: std::collections::BTreeMap<&str, Vec<&ValidationIssue>> =
        std::collections::BTreeMap::new();
    for issue in &issues {
        cat_map.entry(issue.category).or_default().push(issue);
    }

    for (cat, items) in &cat_map {
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

/// Map user-friendly field names to category name fragments.
fn normalize_field(field: &str) -> String {
    match field.to_lowercase().as_str() {
        "dims" | "dimensions" => "header".into(),
        "mode" => "header".into(),
        "map" => "header".into(),
        "ispg" | "spacegroup" => "header".into(),
        "axis" | "mapc" | "mapr" | "maps" => "header".into(),
        "nsymbt" | "ext" | "extended" => "header".into(),
        "nlabl" | "labels" => "header".into(),
        "nversion" | "version" => "header".into(),
        "sampling" => "header".into(),
        "volume-stack" | "volstack" => "header".into(),
        "stats" | "statistics" => "statistics".into(),
        "endian" | "endianness" | "machst" => "endianness".into(),
        "integrity" | "data" | "nan" | "inf" => "data integrity".into(),
        "file" | "filesize" | "size" => "file size".into(),
        "volume" | "vol" => "volume".into(),
        other => other.to_lowercase(),
    }
}
