use half::f16;
use mrc::Header;
use mrc::Mode;
use std::fs::File;
use std::io::Write;

fn create_ball_data_3d(
    mode: Mode,
    width: usize,
    height: usize,
    depth: usize,
    diameter: f32,
) -> Vec<u8> {
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let center_z = depth as f32 / 2.0;
    let radius = diameter / 2.0;
    let mut data = Vec::new();

    match mode {
        Mode::Int8 => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let value = if distance <= radius {
                            (127.0 * (1.0 - distance / radius)) as i8
                        } else {
                            0i8
                        };
                        data.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        Mode::Uint16 => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let value = if distance <= radius {
                            (65535.0 * (1.0 - distance / radius)) as u16
                        } else {
                            0u16
                        };
                        data.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        Mode::Int16 => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let value = if distance <= radius {
                            (32767.0 * (1.0 - distance / radius)) as i16
                        } else {
                            0i16
                        };
                        data.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        Mode::Float32 => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let value = if distance <= radius {
                            1.0 * (1.0 - distance / radius)
                        } else {
                            0.0f32
                        };
                        data.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        Mode::Float16 => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let value = if distance <= radius {
                            1.0 * (1.0 - distance / radius)
                        } else {
                            0.0f32
                        };
                        let f16_value = f16::from_f32(value);
                        data.extend_from_slice(&f16_value.to_le_bytes());
                    }
                }
            }
        }
        Mode::Int16Complex => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let real = if distance <= radius {
                            (32767.0 * (1.0 - distance / radius)) as i16
                        } else {
                            0i16
                        };
                        let imag = 0i16;
                        data.extend_from_slice(&real.to_le_bytes());
                        data.extend_from_slice(&imag.to_le_bytes());
                    }
                }
            }
        }
        Mode::Float32Complex => {
            for z in 0..depth {
                for y in 0..height {
                    for x in 0..width {
                        let dx = x as f32 - center_x;
                        let dy = y as f32 - center_y;
                        let dz = z as f32 - center_z;
                        let distance = (dx * dx + dy * dy + dz *dz).sqrt();

                        let real = if distance <= radius {
                            1.0 * (1.0 - distance / radius)
                        } else {
                            0.0f32
                        };
                        let imag = 0.0f32;
                        data.extend_from_slice(&real.to_le_bytes());
                        data.extend_from_slice(&imag.to_le_bytes());
                    }
                }
            }
        }
        _ => unreachable!(), // All modes are handled above
    }

    data
}

fn create_mrc_file(mode: i32, filename: &str) -> std::io::Result<()> {
    let mut header = Header::new();

    // Set dimensions and basic parameters
    header.nx = 64;
    header.ny = 64;
    header.nz = 64; // 3D for all modes
    header.mode = mode;

    // Set cell dimensions (64 pixels = 64 Ångströms)
    header.xlen = 64.0;
    header.ylen = 64.0;
    header.zlen = 64.0;

    // Set cell angles to 90 degrees
    header.alpha = 90.0;
    header.beta = 90.0;
    header.gamma = 90.0;

    // Set axis correspondence to columns 1,2,3
    header.mapc = 1;
    header.mapr = 2;
    header.maps = 3;

    // Set NSYMBT to 0 (no symmetry data)
    header.nsymbt = 0;

    // Set EXTTYP to "MRCO"
    header.set_exttyp_str("MRCO").unwrap();

    // Set NVERSION to 20141
    header.set_nversion(20141);

    // Calculate data statistics
    let data = create_ball_data_3d(Mode::from_i32(mode).unwrap(), 64, 64, 64, 40.0);

    // Calculate min, max, mean, rms
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut count = 0;

    let mode_enum = Mode::from_i32(mode).unwrap();
    match mode_enum {
        Mode::Int8 => {
            for chunk in data.chunks_exact(1) {
                let val = i8::from_le_bytes([chunk[0]]) as f32;
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        Mode::Uint16 => {
            for chunk in data.chunks_exact(2) {
                let val = u16::from_le_bytes([chunk[0], chunk[1]]) as f32;
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        Mode::Int16 => {
            for chunk in data.chunks_exact(2) {
                let val = i16::from_le_bytes([chunk[0], chunk[1]]) as f32;
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        Mode::Float32 => {
            for chunk in data.chunks_exact(4) {
                let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        Mode::Float16 => {
            for chunk in data.chunks_exact(2) {
                let val = f16::from_le_bytes([chunk[0], chunk[1]]).into();
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        Mode::Int16Complex => {
            for chunk in data.chunks_exact(4) {
                let real = i16::from_le_bytes([chunk[0], chunk[1]]) as f32;
                let imag = i16::from_le_bytes([chunk[2], chunk[3]]) as f32;
                let val = (real * real + imag * imag).sqrt();
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        Mode::Float32Complex => {
            for chunk in data.chunks_exact(8) {
                let real = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let imag = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                let val = (real * real + imag * imag).sqrt();
                min_val = min_val.min(val);
                max_val = max_val.max(val);
                sum += val as f64;
                sum_sq += (val * val) as f64;
                count += 1;
            }
        }
        _ => unreachable!(), // All modes are handled above
    }

    let mean = (sum / count as f64) as f32;
    let rms = ((sum_sq / count as f64 - mean as f64 * mean as f64).max(0.0)).sqrt() as f32;

    header.dmin = min_val;
    header.dmax = max_val;
    header.dmean = mean;
    header.rms = rms;

    // Write file
    let mut file = File::create(filename)?;

    // Write header
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            &header as *const _ as *const u8,
            std::mem::size_of::<Header>(),
        )
    };
    file.write_all(header_bytes)?;

    // Write data
    file.write_all(&data)?;

    println!(
        "Created {}: mode={}, dimensions={}x{}x{}",
        filename, mode, header.nx, header.ny, header.nz
    );

    Ok(())
}

fn main() -> std::io::Result<()> {
    // Create output directory
    std::fs::create_dir_all("mrcs")?;

    // Generate MRC files for supported modes (0-6, 12)
    let modes = [0, 1, 2, 3, 4, 6, 12];

    for &mode in &modes {
        let filename = format!("mrcs/ball_mode_{}.mrc", mode);
        create_mrc_file(mode, &filename)?;
    }

    println!(
        "Successfully generated {} MRC files in mrcs/ directory (modes 0-6, 12)",
        modes.len()
    );

    Ok(())
}
