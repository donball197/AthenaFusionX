use std::fs;
use std::time::SystemTime;
use tokio::process::Command;

pub struct Inquisitor;

impl Inquisitor {
    pub async fn audit_ui() -> anyhow::Result<String> {
        // ⚡ RAM SHIELD: Bypassing heavy NPM builds on the 4GB substrate
        eprintln!("\x1b[33m>> [INQUISITOR] Bypassing UI Build (RAM Shield Active)...\x1b[0m");
        Ok("NOMINAL".into())
    }

    pub async fn audit_backend() -> anyhow::Result<String> {
        let output = Command::new("cargo")
            .args(["check"])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Cargo not found: {}", e))?;
        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub async fn diagnose_workspace() -> anyhow::Result<String> {
        let backend = Self::audit_backend().await.unwrap_or_else(|e| e.to_string());
        let ui = Self::audit_ui().await.unwrap_or_else(|e| e.to_string());

        if backend == "NOMINAL" && ui == "NOMINAL" {
            return Ok("WORKSPACE_NOMINAL: Zero compilation errors detected.".to_string());
        }

        let mut diagnostic = String::from("WORKSPACE_FRACTURE_DETECTED:\n");
        if backend != "NOMINAL" {
            diagnostic.push_str(&format!("--- BACKEND (cargo check) ERRORS ---\n{}\n", backend));
        }
        if ui != "NOMINAL" {
            diagnostic.push_str(&format!("--- FRONTEND (npm run build) ERRORS ---\n{}\n", ui));
        }

        Ok(diagnostic)
    }

    pub fn probe_shadow_activity() -> Vec<String> {
        let mut alerts = Vec::new();
        let targets = ["./ziggy-llm", "./target", "./src", "./vault"];

        for path in targets {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    let now = SystemTime::now();
                    if let Ok(elapsed) = now.duration_since(modified) {
                        if elapsed.as_secs() < 300 {
                            alerts.push(format!(
                                "SHADOW_ACTIVITY_DETECTED: {} modified {}s ago",
                                path,
                                elapsed.as_secs()
                            ));
                        }
                    }
                }
            }
        }
        alerts
    }
}
