use std::fs;
use std::io::Read;
use std::path::Path;

pub fn read_rom_file(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if Path::new(path).exists() {
        let mut file = fs::File::open(path)?;
        let mut rom_data = Vec::new();
        file.read_to_end(&mut rom_data)?;
        return Ok(rom_data);
    }

    Err(format!("ROM file not found: {}", path).into())
}
