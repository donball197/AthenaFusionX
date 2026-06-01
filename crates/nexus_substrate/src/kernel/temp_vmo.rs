//! Temporary VMO Filesystem Bridge for Auditor Agent

use std::io;

#[derive(Default)]
pub struct TempVmoFs;

impl TempVmoFs {
    pub fn new() -> Self {
        Self
    }

    pub fn read_file(&self, _path: &str) -> io::Result<Vec<u8>> {
        // Placeholder — will connect to real VMO layer later
        Ok(b"// TODO: Implement real VMO-backed file read".to_vec())
    }
}
