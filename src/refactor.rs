use crate::auditor;
use std::fs;
use anyhow::{Result, anyhow};

pub struct RefactorEngine;

impl RefactorEngine {
    pub fn new() -> Self { Self }
    pub fn apply_patch(&self, path: &str, new_code: &str) -> Result<()> {
        let temp_path = format!("{}.tmp", path);
        if auditor::validate_patch(&temp_path, new_code) {
            fs::write(path, new_code)?;
            let _ = fs::remove_file(temp_path);
            println!(">> [HIRAM] Refactor successful.");
            Ok(())
        } else {
            let _ = fs::remove_file(temp_path);
            Err(anyhow!("Refactor aborted to prevent kernel panic."))
        }
    }
}
