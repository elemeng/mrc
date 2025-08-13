use mrc::{MrcFile, MrcMmap};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing real MRC files...");

    let mrc_files = ["mrcs/2D_img.mrc", "mrcs/map.mrc", "mrcs/movie.mrc"];

    for file_path in mrc_files {
        println!("\n=== Testing {} ===", file_path);

        match MrcFile::open(file_path) {
            Ok(file) => {
                let header = file.header();
                println!("✅ Successfully opened");
                println!(
                    "Dimensions: {}x{}x{} = {} voxels",
                    header.nx,
                    header.ny,
                    header.nz,
                    header.nx * header.ny * header.nz
                );
                println!("Mode: {} (data type)", header.mode);
                println!(
                    "Cell dimensions: {:.2}×{:.2}×{:.2} Å",
                    header.xlen, header.ylen, header.zlen
                );
                println!("Extended header: {} bytes", header.nsymbt);
                println!(
                    "Data range: {:.3} to {:.3} (mean: {:.3})",
                    header.dmin, header.dmax, header.dmean
                );

                // Test reading the data
                match file.read_data() {
                    Ok(data) => println!("✅ Read {} bytes of data", data.len()),
                    Err(e) => println!("❌ Failed to read data: {}", e),
                }
            }
            Err(e) => println!("❌ Failed to open: {}", e),
        }

        #[cfg(feature = "mmap")]
        {
            match MrcMmap::open(file_path) {
                Ok(mmap) => {
                    println!("✅ Mmap opened successfully");
                    println!("Data size: {} bytes", mmap.data().len());
                }
                Err(e) => println!("❌ Mmap failed: {}", e),
            }
        }
    }

    Ok(())
}
