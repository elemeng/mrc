use mrc::{Header, MrcFile, MrcMmap};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        let file = MrcFile::open("test.mrc")?;// file descriptor, no data loaded yet.
        let header = file.header(); // read header from file, only header's xxx bytes loaded into memory.

        println!("{:?}", header);

        let data = vec![0u8; 1024]; // data buffer allocated in memory, ready for operations.
        println!("Loaded data size: {} bytes", data.len());

        Ok(())
    }
}
