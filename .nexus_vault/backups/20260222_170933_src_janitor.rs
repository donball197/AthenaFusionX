use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct Janitor;

impl Janitor {
    /// Calculates the total size of a directory in bytes
    pub fn get_dir_size<P: AsRef<Path>>(path: P) -> u64 {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.metadata().ok())
            .filter(|metadata| metadata.is_file())
            .map(|metadata| metadata.len())
            .sum()
    }

    /// Automatically purges the target folder if it exceeds a 1GB threshold
    pub fn cleanup_build_artifacts() -> Result<String, String> {
        let target_path = "target";
        if Path::new(target_path).exists() {
            let size_bytes = Self::get_dir_size(target_path);
            let size_mb = size_bytes / 1024 / 1024;

            // Threshold: 1000MB (1GB) - Vital for 4GB Duet storage
            if size_mb > 1000 {
                println!(">> [JANITOR] Target folder is {}MB. Purging for Disk safety...", size_mb);
                let _ = fs::remove_dir_all(target_path);
                return Ok(format!("Purged {}MB of build artifacts.", size_mb));
            }
            return Ok(format!("Disk Healthy: Target is {}MB.", size_mb));
        }
        Ok("Target folder not found.".into())
    }
}
