use std::process::Command;
use crate::nexus_error::NexusError;

pub struct Inquisitor;

impl Inquisitor {
    /// audit_ui: Runs the actual Svelte build and captures the REAL error.
    pub fn audit_ui() -> Result<String, NexusError> {
        println!("\x1b[33m>> [INQUISITOR] Executing UI Build Interrogation...\x1b[0m");
        
        // We use output() instead of status() to capture the text
        let output = Command::new("npm")
            .args(["run", "build"])
            .current_dir("ui")
            .output()
            .map_err(|_| NexusError::Other("NPM binary not found in path".into()))?;

        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            // Combine both because some builders output errors to stdout
            Ok(format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr))
        }
    }

    pub fn audit_backend() -> Result<String, NexusError> {
        let output = Command::new("cargo")
            .args(["check"])
            .output()
            .map_err(|_| NexusError::Other("Cargo not found".into()))?;

        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
