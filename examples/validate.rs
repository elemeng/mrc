use mrc::Header;
use std::mem;

fn main() {
    println!("Header size: {} bytes", mem::size_of::<Header>());
    println!("Header alignment: {} bytes", mem::align_of::<Header>());

    // Calculate individual field sizes
    let fields = [
        ("nx", mem::size_of::<i32>()),
        ("ny", mem::size_of::<i32>()),
        ("nz", mem::size_of::<i32>()),
        ("mode", mem::size_of::<i32>()),
        ("nxstart", mem::size_of::<i32>()),
        ("nystart", mem::size_of::<i32>()),
        ("nzstart", mem::size_of::<i32>()),
        ("mx", mem::size_of::<i32>()),
        ("my", mem::size_of::<i32>()),
        ("mz", mem::size_of::<i32>()),
        ("xlen", mem::size_of::<f32>()),
        ("ylen", mem::size_of::<f32>()),
        ("zlen", mem::size_of::<f32>()),
        ("alpha", mem::size_of::<f32>()),
        ("beta", mem::size_of::<f32>()),
        ("gamma", mem::size_of::<f32>()),
        ("mapc", mem::size_of::<i32>()),
        ("mapr", mem::size_of::<i32>()),
        ("maps", mem::size_of::<i32>()),
        ("dmin", mem::size_of::<f32>()),
        ("dmax", mem::size_of::<f32>()),
        ("dmean", mem::size_of::<f32>()),
        ("ispg", mem::size_of::<i32>()),
        ("nsymbt", mem::size_of::<i32>()),
        ("extra", mem::size_of::<[u8; 100]>()),
        ("origin", mem::size_of::<[f32; 3]>()),
        ("map", mem::size_of::<[u8; 4]>()),
        ("machst", mem::size_of::<[u8; 4]>()),
        ("rms", mem::size_of::<f32>()),
        ("nlabl", mem::size_of::<i32>()),
        ("label", mem::size_of::<[u8; 800]>()),
    ];

    let total: usize = fields.iter().map(|(_, size)| size).sum();
    println!("\nField breakdown:");
    for (name, size) in fields {
        println!("{:12}: {} bytes", name, size);
    }
    println!("Total: {} bytes", total);
}
