use std::process::Command;

pub struct Inquisitor;

impl Inquisitor {
    pub fn audit_ui() -> anyhow::Result<String> {
        println!("\x1b[33m>> [INQUISITOR] Executing UI Build Interrogation...\x1b[0m");
        let output = Command::new("npm").args(["run", "build"]).current_dir("ui").output()
            .map_err(|e| anyhow::anyhow!("NPM binary not found: {}", e))?;

        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr))
        }
    }

    pub fn audit_backend() -> anyhow::Result<String> {
        let output = Command::new("cargo").args(["check"]).output()
            .map_err(|e| anyhow::anyhow!("Cargo not found: {}", e))?;
        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
