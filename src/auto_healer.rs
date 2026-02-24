use std::path::Path;
use tokio::process::Command;

pub struct AutoHealer;

impl AutoHealer {
    pub async fn run_diagnostic() -> Option<String> {
        // 1. Check Cargo.toml existence
        if !Path::new("Cargo.toml").exists() {
            return Some("CRITICAL: Cargo.toml missing.".to_string());
        }

        // 2. Quick Syntax Check (Cargo Check)
        // We run this quietly to avoid spamming logs
        let output = Command::new("cargo")
            .arg("check")
            .output()
            .await;

        match output {
            Ok(out) => {
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    // Return first 200 chars of error to save tokens
                    return Some(format!("BUILD_FAIL: {}", &err.chars().take(200).collect::<String>()));
                }
            }
            Err(_) => return Some("CRITICAL: Cargo binary not found.".to_string()),
        }

        None // System Healthy
    }
}
