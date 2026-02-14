use crate::auditor;
use std::fs;
use std::path::Path;
use anyhow::{Result, anyhow};

pub struct RefactorEngine;

impl RefactorEngine {
    pub fn new() -> Self { Self }

    pub fn apply_patch(&self, path: &str, new_code: &str) -> Result<()> {
        let temp_path = format!("{}.tmp", path);
        
        let extension = Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("txt");

        // Convert Result to bool using .is_ok()
        if auditor::validate_patch(&temp_path, new_code, extension).is_ok() {
            fs::write(path, new_code)?;
            let _ = fs::remove_file(temp_path);
            println!(">> [FORGE] Refactor successful: {}", path);
            Ok(())
        } else {
            let _ = fs::remove_file(temp_path);
            Err(anyhow!("Refactor aborted: Code failed [.{}] validation.", extension))
        }
    }
}
