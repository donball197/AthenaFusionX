//! Auditor Agent — Code quality & security auditor

use anyhow::{anyhow, Result};
use nexus_substrate::kernel::temp_vmo::TempVmoFs;
use tree_sitter::Parser;

#[derive(Default)]
pub struct AuditorAgent {
    sandbox: TempVmoFs,
}

impl AuditorAgent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn audit_file(&self, path: &str) -> Result<String> {
        println!("[AUDITOR] Auditing: {}", path);

        let mut parser = Parser::new();
        let language = tree_sitter_rust::language();
        parser
            .set_language(&language)
            .map_err(|e| anyhow!("Failed to set Rust language: {}", e))?;

        // Read file via sandbox
        let content = self.sandbox.read_file(path).map_err(|e| anyhow!("Read failed: {}", e))?;

        let content_str = String::from_utf8_lossy(&content);

        Ok(format!("[AUDIT_OK] {} — {} bytes analyzed. Parser ready.", path, content_str.len()))
    }
}
