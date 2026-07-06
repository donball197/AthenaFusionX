/**
 * Copyright 2026 Donald Ball
 * Project Nexus Singularity - Inquisitor v4.3.5
 */

use std::process::Command;
use std::fs;
use std::time::SystemTime;

pub struct Inquisitor;

impl Inquisitor {
    pub fn audit_ui() -> anyhow::Result<String> {
        eprintln!("\x1b[33m>> [INQUISITOR] Executing UI Build Interrogation...\x1b[0m");
        let output = Command::new("npm")
            .args(["--prefix", "ui", "run", "build"])
            .output()
            .map_err(|e| anyhow::anyhow!("NPM binary not found: {}", e))?;

        if output.status.success() { Ok("NOMINAL".into()) }
        else { Ok(String::from_utf8_lossy(&output.stderr).to_string()) }
    }

    pub fn audit_backend() -> anyhow::Result<String> {
        let output = Command::new("cargo").args(["check"]).output()
            .map_err(|e| anyhow::anyhow!("Cargo not found: {}", e))?;
        if output.status.success() { Ok("NOMINAL".into()) }
        else { Ok(String::from_utf8_lossy(&output.stderr).to_string()) }
    }

    // NEW: Shadow Activity Detection
    pub fn probe_shadow_activity() -> Vec<String> {
        let mut alerts = Vec::new();
        let targets = ["./ziggy-llm", "./target", "./src", "./vault"];

        for path in targets {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    let now = SystemTime::now();
                    if let Ok(elapsed) = now.duration_since(modified) {
                        if elapsed.as_secs() < 300 { // Changed in the last 5 minutes
                            alerts.push(format!("SHADOW_ACTIVITY_DETECTED: {} modified {}s ago", path, elapsed.as_secs()));
                        }
                    }
                }
            }
        }
        alerts
    }
}
