use anyhow::{anyhow, Result};
use std::process::Command;

pub struct GemmaReflex;

impl GemmaReflex {
    pub fn init() -> Self {
        Self {}
    }

    pub fn invoke(&self, prompt: &str) -> Result<String> {
        // Trigger Zig Substrate with the prompt
        // Process is limited by the system governor to stay under 1GB
        let output = Command::new("./substrate/zig-out/bin/nexus_substrate")
            .arg(prompt)
            .output()
            .map_err(|e| anyhow!("Substrate Link Error: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Substrate OOM or Fault"))
        }
    }
}
