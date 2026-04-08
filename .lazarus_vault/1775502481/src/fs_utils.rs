use std::path::Path;
use std::fs;

pub fn check_dir(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn read_file_safe(path: &str) -> std::io::Result<String> {
    fs::read_to_string(path)
}
