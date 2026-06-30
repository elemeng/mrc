//! Invert voxel contrast in an MRC file.
//!
//! Negates every voxel value (v → −v), flipping black-on-white to
//! white-on-black and vice versa.  The output is written as Float32
//! regardless of the input mode.
//!
//! Usage:
//! ```text
//! mrc-invert <input.mrc> <output.mrc>
//! ```

use mrc::{create, open, VoxelBlock};
use std::env;
use std::process;

fn usage() {
    eprintln!("mrc-invert v{} — MRC contrast inverter", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Negates every voxel value (v → −v) to flip black-on-white to");
    eprintln!("white-on-black and vice versa.  Reads any mode, writes Float32.");
    eprintln!("Auto-detects gzip/bzip2 compression on input.");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  mrc-invert <input> <output>");
    eprintln!();
    eprintln!("ARGS:");
    eprintln!("  <input>        Path to input MRC file");
    eprintln!("  <output>       Path for the inverted output (uncompressed Float32)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  -h, --help     Print this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  mrc-invert protein.mrc protein_inverted.mrc");
    eprintln!("  mrc-invert density.mrc.gz inverted.mrc");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 || args[1] == "--help" || args[1] == "-h" {
        usage();
        process::exit(0);
    }

    let input = &args[1];
    let output = &args[2];

    // Open input (auto-detects gzip/bzip2)
    let reader = match open(input) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error opening '{}': {}", input, e);
            process::exit(2);
        }
    };

    let shape = reader.shape();
    let mode = reader.mode();

    eprintln!("Input:  {} × {} × {}, mode={:?}", shape.nx, shape.ny, shape.nz, mode);
    eprintln!("Output: {} × {} × {}, mode=Float32 (inverted)", shape.nx, shape.ny, shape.nz);

    // Create output writer
    let mut writer = match create(output)
        .shape([shape.nx, shape.ny, shape.nz])
        .mode::<f32>()
        .finish()
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error creating '{}': {}", output, e);
            process::exit(2);
        }
    };

    // Process slice by slice
    let total_slices = shape.nz;
    for (z, result) in reader.slices_f32().enumerate() {
        let block = match result {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error reading slice {}: {}", z, e);
                process::exit(2);
            }
        };

        // Negate every voxel to invert contrast
        let inverted: Vec<f32> = block.data.iter().map(|&v| -v).collect();

        let out_block = match VoxelBlock::new(block.offset, block.shape, inverted) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error creating output block: {}", e);
                process::exit(2);
            }
        };

        if let Err(e) = writer.write_block(&out_block) {
            eprintln!("Error writing slice {}: {}", z, e);
            process::exit(2);
        }

        if (z + 1) % 100 == 0 || z + 1 == total_slices {
            eprintln!("  Progress: {}/{} slices", z + 1, total_slices);
        }
    }

    // Update header statistics and finalize
    if let Err(e) = writer.update_header_stats() {
        eprintln!("Warning: could not update header stats: {}", e);
    }
    if let Err(e) = writer.finalize() {
        eprintln!("Error finalizing '{}': {}", output, e);
        process::exit(2);
    }

    eprintln!("Done: inverted {}", output);
}
