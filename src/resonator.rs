use memmap2::Mmap;
use std::fs::File;
use std::io;

pub struct Resonator;
impl Resonator {
    pub fn scan_vault(path: &str) -> io::Result<String> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let preview = String::from_utf8_lossy(&mmap[0..std::cmp::min(mmap.len(), 100)]);
        Ok(format!(">> [RESONATOR] Mapped 5GB Object | Size: {} bytes | Data: {}", mmap.len(), preview))
    }
}
