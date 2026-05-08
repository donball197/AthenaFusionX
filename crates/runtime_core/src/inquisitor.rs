// inquisitor.rs — v1.1 [UI_AUDIT_RESTORED]
// Autonomous workspace auditor. Called by SelfHealingEngine on each cycle.
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tokio::process::Command;

pub struct Inquisitor;

impl Inquisitor {
    /// audit_ui: Checks for missing critical Svelte/UI files.
    pub async fn audit_ui() -> anyhow::Result<String> {
        let critical_paths = ["ui/src/App.svelte", "ui/package.json", "ui/index.html"];

        for path in critical_paths {
            if !Path::new(path).exists() {
                return Ok(format!("FRACTURE: Missing critical UI file: {}", path));
            }
        }

        if !Path::new("ui/node_modules").exists() {
            return Ok("FRACTURE: UI dependencies (node_modules) missing. Run npm install.".into());
        }

        Ok("NOMINAL".into())
    }

    /// audit_backend: Runs `cargo check` and returns errors or "NOMINAL".
    pub async fn audit_backend() -> anyhow::Result<String> {
        let output = Command::new("cargo")
            .args(["check"])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Cargo check failed: {}", e))?;

        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// diagnose_workspace: Runs both audits and returns a combined report.
    pub async fn diagnose_workspace() -> anyhow::Result<String> {
        let backend = Self::audit_backend().await.unwrap_or_else(|e| e.to_string());
        let ui = Self::audit_ui().await.unwrap_or_else(|e| e.to_string());

        if backend == "NOMINAL" && ui == "NOMINAL" {
            return Ok(
                "WORKSPACE_NOMINAL: Integrity verified across Rust and Svelte domains.".into()
            );
        }

        let mut diagnostic = String::from("WORKSPACE_FRACTURE_DETECTED:\n");
        if backend != "NOMINAL" {
            diagnostic.push_str(&format!("--- BACKEND ERRORS ---\n{}\n", backend));
        }
        if ui != "NOMINAL" {
            diagnostic.push_str(&format!("--- FRONTEND ERRORS ---\n{}\n", ui));
        }

        Ok(diagnostic)
    }

    /// probe_shadow_activity: Detects files modified in the last 60 seconds.
    pub fn probe_shadow_activity() -> Vec<String> {
        let mut alerts = Vec::new();
        let targets = ["./src", "./ui/src"];

        for path in targets {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                        if elapsed.as_secs() < 60 {
                            alerts.push(format!(
                                "SHADOW_ACTIVITY: {} modified recently ({}s ago)",
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
