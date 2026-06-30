//! MRC header inspection CLI with semantic interpretation.
//!
//! Usage:
//! ```text
//! mrc-header <file.mrc>
//! mrc-header --permissive <file.mrc>
//! ```

use mrc::{Mode, Reader};
use std::env;
use std::process;

fn usage() {
    eprintln!("Usage: mrc-header [--permissive] <file.mrc>");
    eprintln!();
    eprintln!("Options:");
    eprintln!(
        "  --permissive   Open in permissive mode (warn instead of error on non-critical issues)"
    );
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
            "--permissive" => permissive = true,
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

    let header = reader.header();
    let mode = reader.mode();
    let shape = reader.shape();
    let endian = reader.endian();
    let data_size = header.data_size().unwrap_or(0);

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              MRC Header Summary                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ── File Identity ──
    println!("── File Identity ──");
    print_map_field("MAP identifier", &header.map);
    println!(
        "  Format version:     MRC-2014 (NVERSION {})",
        header.nversion()
    );
    println!(
        "  File size:          ~{} bytes",
        1024 + header.nsymbt.max(0) as usize + data_size
    );
    println!();

    // ── Volume Dimensions ──
    println!("── Volume Dimensions ──");
    println!(
        "  Grid size:          {} × {} × {} voxels",
        header.nx, header.ny, header.nz
    );
    println!(
        "  Total voxels:       {}",
        shape.total_voxels().unwrap_or(0)
    );
    let vol_type = if header.is_single_image() {
        "Single 2D image"
    } else if header.is_image_stack() {
        "Image stack"
    } else if header.is_volume_stack() {
        "Volume stack"
    } else {
        "3D volume"
    };
    println!("  Volume type:        {}", vol_type);
    if header.is_volume_stack() {
        let nvol = if header.mz > 0 {
            header.nz / header.mz
        } else {
            0
        };
        println!("  Sub-volumes:        {} × {} slices each", nvol, header.mz);
    }
    println!();

    // ── Data Type ──
    println!("── Data Type ──");
    let mode_label = match mode {
        Mode::Int8 => "Signed 8-bit integer",
        Mode::Int16 => "Signed 16-bit integer",
        Mode::Float32 => "32-bit float",
        Mode::Int16Complex => "Complex 16-bit integer (real + imag i16)",
        Mode::Float32Complex => "Complex 32-bit float (real + imag f32)",
        Mode::Uint16 => "Unsigned 16-bit integer",
        Mode::Float16 => "16-bit float (half-precision)",
        Mode::Packed4Bit => "4-bit packed (2 values per byte)",
        _ => "Unknown mode",
    };
    println!("  Mode:               {} ({})", mode_label, mode.as_i32());
    println!("  Bytes per voxel:    {}", mode.byte_size());
    println!("  Total data size:    {} bytes", data_size);
    println!();

    // ── Endianness ──
    println!("── Endianness ──");
    let (endian_label, endian_note) = match endian {
        mrc::FileEndian::LittleEndian => ("Little-endian", "matches most modern systems"),
        mrc::FileEndian::BigEndian => ("Big-endian", "non-native on x86_64 / ARM"),
    };
    let machine_stamp = format!(
        "{:02X} {:02X} {:02X} {:02X}",
        header.machst[0], header.machst[1], header.machst[2], header.machst[3]
    );
    println!("  MACHST stamp:       {}", machine_stamp);
    println!("  Byte order:         {} ({})", endian_label, endian_note);
    println!();

    // ── Cell Geometry ──
    println!("── Cell Geometry ──");
    println!(
        "  Cell lengths (Å):   x={:.3}, y={:.3}, z={:.3}",
        header.xlen, header.ylen, header.zlen
    );
    println!(
        "  Cell angles (°):    α={:.1}, β={:.1}, γ={:.1}",
        header.alpha, header.beta, header.gamma
    );
    let vs = header.voxel_size();
    println!(
        "  Voxel size (Å/px):  x={:.4}, y={:.4}, z={:.4}",
        vs[0], vs[1], vs[2]
    );
    println!(
        "  Sampling (mx/my/mz): {} × {} × {}",
        header.mx, header.my, header.mz
    );
    println!(
        "  Origin (nx/y/zstart): {} {} {}",
        header.nxstart, header.nystart, header.nzstart
    );
    println!();

    // ── Axis Mapping ──
    println!("── Axis Order ──");
    let axis_name = |a: i32| match a {
        1 => "X",
        2 => "Y",
        3 => "Z",
        _ => "?",
    };
    println!(
        "  Column (fast) axis:  MAPC={} ({})",
        header.mapc,
        axis_name(header.mapc)
    );
    println!(
        "  Row (medium) axis:   MAPR={} ({})",
        header.mapr,
        axis_name(header.mapr)
    );
    println!(
        "  Section (slow) axis: MAPS={} ({})",
        header.maps,
        axis_name(header.maps)
    );
    println!();

    // ── Space Group ──
    println!("── Space Group (ISPG) ──");
    let ispg_desc: String = if header.ispg == 0 {
        "Image or image stack (no crystallographic symmetry)".into()
    } else if (1..=230).contains(&header.ispg) {
        "Crystallographic space group".into()
    } else if (401..=630).contains(&header.ispg) {
        let real_ispg = header.ispg - 400;
        if (1..=230).contains(&real_ispg) {
            format!(
                "Volume stack (ISPG = {} + 400, space group {})",
                real_ispg, real_ispg
            )
        } else {
            format!("Volume stack (ISPG = {})", header.ispg)
        }
    } else {
        format!("Non-standard ({})", header.ispg)
    };
    println!("  ISPG:               {} — {}", header.ispg, ispg_desc);
    println!();

    // ── Data Statistics ──
    println!("── Data Statistics ──");
    let stat_undetermined = header.dmin > header.dmax;
    let fmt_stat = |v: f32, label: &str| {
        let undetermined = match label {
            "dmin" | "dmax" => stat_undetermined,
            "dmean" => stat_undetermined || header.dmean < header.dmin.min(header.dmax),
            "rms" => header.rms < 0.0,
            _ => false,
        };
        if undetermined {
            format!("{:.6} (not well-determined)", v)
        } else {
            format!("{:.6}", v)
        }
    };
    println!("  dmin:               {}", fmt_stat(header.dmin, "dmin"));
    println!("  dmax:               {}", fmt_stat(header.dmax, "dmax"));
    println!("  dmean:              {}", fmt_stat(header.dmean, "dmean"));
    println!("  rms:                {}", fmt_stat(header.rms, "rms"));
    println!();

    // ── Extended Header ──
    println!("── Extended Header ──");
    let ext_size = header.nsymbt.max(0) as usize;
    println!("  NSYMBT:             {} bytes", ext_size);
    if let Ok(exttyp) = header.exttyp_str() {
        let exttyp_clean = exttyp.trim_end_matches('\0');
        if !exttyp_clean.is_empty() {
            println!("  EXTTYP:             {}", exttyp_clean);
            let ext_desc = match exttyp_clean {
                "CCP4" => "CCP4 symmetry records",
                "MRCO" => "MRC extended header (IMOD)",
                "SERI" => "SerialEM extended header",
                "AGAR" => "FEI/Applied Microscopy extended header",
                "FEI1" => "FEI Titan extended header (768 bytes/record)",
                "FEI2" => "FEI Titan extended header v2 (888 bytes/record)",
                "HDF5" => "HDF5 dataset (external)",
                _ => "Proprietary or unknown format",
            };
            println!("           {}", ext_desc);
        }
    }
    println!();

    // ── Labels ──
    let labels = header.get_labels();
    if !labels.is_empty() {
        println!("── Labels ({}) ──", labels.len());
        for (i, label) in labels.iter().enumerate() {
            println!("  [{}] {}", i, label);
        }
        println!();
    }

    // ── Origin ──
    println!("── Origin ──");
    println!(
        "  Origin (x, y, z):   {:.1}, {:.1}, {:.1}",
        header.origin[0], header.origin[1], header.origin[2]
    );
    println!();

    // ── Validation ──
    println!("── Validation ──");
    match header.validate_detailed() {
        Ok(()) => println!("  Header structure:   ✅ Valid"),
        Err(e) => println!("  Header structure:   ❌ {}", e),
    }
    match reader.validate_header_stats() {
        Ok(()) => println!("  Data statistics:    ✅ Match header"),
        Err(_) => println!("  Data statistics:    ⚠️  Mismatch (run mrc-validate for details)"),
    }

    if !warnings.is_empty() {
        println!();
        println!("── Warnings ──");
        for w in &warnings {
            println!("  ⚠️  {}", w);
        }
    }
}

/// Pretty-print a 4-byte MAP field.
fn print_map_field(label: &str, map: &[u8; 4]) {
    let ascii = String::from_utf8_lossy(map);
    let hex = format!(
        "{:02X} {:02X} {:02X} {:02X}",
        map[0], map[1], map[2], map[3]
    );
    println!("  {:<18} {} ({})", label, ascii, hex);
}
