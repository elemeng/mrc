//! MRC header inspector with key:value output and optional validation.
//!
//! Usage:
//! ```text
//! mrc-header [--permissive] [--force] <file.mrc>
//! ```

use mrc::validate::{Severity, validate_reader};
use mrc::{Mode, Reader};
use std::env;
use std::process;

fn usage() {
    eprintln!(
        "mrc-header v{} — MRC header inspector",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!();
    eprintln!("Reads an MRC file and prints header fields in key: value format.");
    eprintln!("By default validates each field inline. Use --force to skip validation.");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  mrc-header [OPTIONS] <file>");
    eprintln!();
    eprintln!("ARGS:");
    eprintln!("  <file>         Path to an MRC file (.mrc, .mrc.gz, .mrc.bz2)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  -p, --permissive   Open in permissive mode");
    eprintln!("  -f, --force        Skip validation, show raw values only");
    eprintln!("  -h, --help         Print this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  mrc-header protein.mrc");
    eprintln!("  mrc-header --force protein.mrc");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        process::exit(0);
    }

    let mut permissive = false;
    let mut force = false;
    let mut path: Option<&str> = None;

    for arg in &args {
        match arg.as_str() {
            "--permissive" | "-p" => permissive = true,
            "--force" | "-f" => force = true,
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                usage();
                process::exit(2);
            }
            _ => {
                if path.is_some() {
                    eprintln!("Only one file can be inspected at a time");
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

    let (reader, warnings) = match if permissive {
        Reader::open_permissive(path)
    } else {
        Reader::open(path).map(|r| (r, Vec::new()))
    } {
        Ok(rw) => rw,
        Err(e) => {
            eprintln!("Error opening '{}': {}", path, e);
            process::exit(2);
        }
    };

    // Determine compression for the report
    let compression = match mrc::detect_compression(path) {
        Ok(mrc::CompressionType::Plain) => "plain",
        #[cfg(feature = "gzip")]
        Ok(mrc::CompressionType::Gzip) => "gzip",
        #[cfg(feature = "bzip2")]
        Ok(mrc::CompressionType::Bzip2) => "bzip2",
        _ => "unknown",
    };

    // Run validation (unless --force) — reuses the already-open reader
    let report = if force {
        None
    } else {
        match validate_reader(&reader, path, compression, &warnings) {
            Ok(r) => {
                let has_errors = !r.is_valid();
                Some((r.issues, has_errors))
            }
            Err(_) => None,
        }
    };

    let issues_map: std::collections::HashMap<&str, &mrc::validate::ValidationIssue> = report
        .as_ref()
        .map(|(issues, _)| {
            issues
                .iter()
                .filter(|i| i.severity == Severity::Error)
                .map(|i| (i.category, i))
                .collect()
        })
        .unwrap_or_default();

    let has_errors = report.as_ref().map(|(_, e)| *e).unwrap_or(false);

    let header = reader.header();
    let mode = reader.mode();
    let endian = reader.endian();
    let data_size = header.data_size().unwrap_or(0);

    // ── Identity ──
    println!("## Identity");
    print_map("map", &header.map);
    let header_err = if issues_map.contains_key("Header") {
        " ❌"
    } else {
        ""
    };
    println!("nversion:      {}{}", header.nversion(), header_err);
    println!("compression:   auto-detected");

    // ── Dimensions ──
    println!("\n## Dimensions");
    println!("nx:            {}", header.nx);
    println!("ny:            {}", header.ny);
    println!("nz:            {}", header.nz);
    let vol_type = if header.is_single_image() {
        "single image"
    } else if header.is_image_stack() {
        "image stack"
    } else if header.is_volume_stack() {
        let nvol = if header.mz > 0 {
            header.nz / header.mz
        } else {
            0
        };
        println!("sub-volumes:   {} × {} slices", nvol, header.mz);
        "volume stack"
    } else {
        "3D volume"
    };
    let vol_err = if issues_map.contains_key("Volume") {
        " ❌"
    } else {
        ""
    };
    println!("volume-type:   {}{}", vol_type, vol_err);

    // ── Data type ──
    println!("\n## Data type");
    let mode_label = match mode {
        Mode::Int8 => "int8",
        Mode::Int16 => "int16",
        Mode::Float32 => "float32",
        Mode::Int16Complex => "complex-int16",
        Mode::Float32Complex => "complex-float32",
        Mode::Uint16 => "uint16",
        Mode::Float16 => "float16",
        Mode::Packed4Bit => "packed-4bit",
        _ => "unknown",
    };
    println!(
        "mode:          {} ({}){}",
        mode_label,
        mode.as_i32(),
        header_err
    );
    println!("bytes/voxel:   {}", mode.byte_size());
    println!("data-bytes:    {}", data_size);

    // ── Endianness ──
    println!("\n## Endianness");
    let (endian_label, endian_note) = match endian {
        mrc::FileEndian::LittleEndian => ("little-endian", "native"),
        mrc::FileEndian::BigEndian => ("big-endian", "non-native"),
    };
    let machst = format!(
        "{:02X} {:02X} {:02X} {:02X}",
        header.machst[0], header.machst[1], header.machst[2], header.machst[3]
    );
    let endian_err = if issues_map.contains_key("Endianness") {
        " ❌"
    } else {
        ""
    };
    println!("machst:        {}{}", machst, endian_err);
    println!("byte-order:    {} ({})", endian_label, endian_note);

    // ── Cell geometry ──
    println!("\n## Cell geometry");
    println!(
        "cell-lengths:  {:.3} {:.3} {:.3} Å",
        header.xlen, header.ylen, header.zlen
    );
    println!(
        "cell-angles:   {:.1} {:.1} {:.1}°",
        header.alpha, header.beta, header.gamma
    );
    let vs = header.voxel_size();
    println!("voxel-size:    {:.4} {:.4} {:.4} Å/px", vs[0], vs[1], vs[2]);
    println!("sampling:      {} {} {}", header.mx, header.my, header.mz);
    println!(
        "nstart:        {} {} {}",
        header.nxstart, header.nystart, header.nzstart
    );

    // ── Axis order ──
    println!("\n## Axis order");
    let axis = |a: i32| match a {
        1 => "X",
        2 => "Y",
        3 => "Z",
        _ => "?",
    };
    println!("mapc:          {} ({})", header.mapc, axis(header.mapc));
    println!("mapr:          {} ({})", header.mapr, axis(header.mapr));
    println!("maps:          {} ({})", header.maps, axis(header.maps));

    // ── Space group ──
    println!("\n## Space group");
    let ispg_desc = if header.ispg == 0 {
        "image stack"
    } else if (1..=230).contains(&header.ispg) {
        "crystallographic"
    } else if (401..=630).contains(&header.ispg) {
        let r = header.ispg - 400;
        if (1..=230).contains(&r) {
            "volume stack (space group)"
        } else {
            "volume stack"
        }
    } else {
        "non-standard"
    };
    println!(
        "ispg:          {} ({}){}",
        header.ispg, ispg_desc, header_err
    );

    // ── Statistics ──
    println!("\n## Statistics");
    let stat_unset = header.dmin > header.dmax;
    let fmt = |v: f32, label: &str| {
        let undetermined = match label {
            "dmin" | "dmax" => stat_unset,
            "dmean" => stat_unset || header.dmean < header.dmin.min(header.dmax),
            "rms" => header.rms < 0.0,
            _ => false,
        };
        if undetermined {
            format!("{:.6} (unset)", v)
        } else {
            format!("{:.6}", v)
        }
    };
    println!("dmin:          {}", fmt(header.dmin, "dmin"));
    println!("dmax:          {}", fmt(header.dmax, "dmax"));
    println!("dmean:         {}", fmt(header.dmean, "dmean"));
    println!("rms:           {}", fmt(header.rms, "rms"));

    // Check for stats validation issues
    let stats_issue = issues_map.get("Statistics");
    if let Some(issue) = stats_issue {
        println!("stats-check:   FAIL ❌ {}", issue.message);
    } else if !force {
        println!("stats-check:   OK");
    }

    // ── Extended header ──
    println!("\n## Extended header");
    let ext_size = header.nsymbt.max(0) as usize;
    println!("nsymbt:        {} bytes", ext_size);
    if let Ok(exttyp) = header.exttyp_str() {
        let clean = exttyp.trim_end_matches('\0');
        if !clean.is_empty() {
            let desc = match clean {
                "CCP4" => "CCP4 symmetry",
                "MRCO" => "IMOD",
                "SERI" => "SerialEM",
                "AGAR" => "FEI/AMI",
                "FEI1" => "FEI Titan v1",
                "FEI2" => "FEI Titan v2",
                "HDF5" => "HDF5",
                _ => "proprietary",
            };
            println!("exttyp:        {} ({})", clean, desc);
        }
    }

    // ── Labels ──
    let labels = header.get_labels();
    if !labels.is_empty() {
        println!("\n## Labels ({})", labels.len());
        for (i, label) in labels.iter().enumerate() {
            println!("label[{}]:      {}", i, label);
        }
    }

    // ── Origin ──
    println!("\n## Origin");
    println!(
        "origin:        {:.1} {:.1} {:.1}",
        header.origin[0], header.origin[1], header.origin[2]
    );

    // ── Validation summary ──
    if !force {
        println!("\n## Validation");
        if has_errors {
            println!("status:        FAIL");
            // Print all error messages
            if let Some((issues, _)) = report.as_ref() {
                for issue in issues.iter().filter(|i| i.severity == Severity::Error) {
                    println!("  ❌ [{}] {}", issue.category, issue.message);
                }
                for issue in issues.iter().filter(|i| i.severity == Severity::Warning) {
                    println!("  ⚠️  [{}] {}", issue.category, issue.message);
                }
            }
        } else {
            println!("status:        OK");
        }
    }

    // ── Warnings (permissive mode) ──
    if !warnings.is_empty() {
        println!("\n## Warnings");
        for w in &warnings {
            println!("  ⚠️  {}", w);
        }
    }
}

fn print_map(label: &str, map: &[u8; 4]) {
    let ascii = String::from_utf8_lossy(map);
    let hex = format!(
        "{:02X} {:02X} {:02X} {:02X}",
        map[0], map[1], map[2], map[3]
    );
    println!("{}: {} ({})", label, ascii, hex);
}
