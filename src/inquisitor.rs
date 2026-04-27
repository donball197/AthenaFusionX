// src/inquisitor.rs — v1.1 [UI_AUDIT_RESTORED]
use std::fs;
use std::time::SystemTime;
use tokio::process::Command;
use std::path::Path;

pub struct Inquisitor;

impl Inquisitor {
    /// audit_ui: Performs a lightweight structural audit of the Svelte substrate.
    /// Replaces the legacy "RAM Shield" bypass with actual integrity checks.
    pub async fn audit_ui() -> anyhow::Result<String> {
        let critical_paths = ["ui/src/App.svelte", "ui/package.json", "ui/index.html"];
        
        for path in critical_paths {
            if !Path::new(path).exists() {
                return Ok(format!("FRACTURE: Missing critical UI file: {}", path));
            }
        }

        // Lightweight check: ensure node_modules isn't purged
        if !Path::new("ui/node_modules").exists() {
            return Ok("FRACTURE: UI dependencies (node_modules) missing. Run npm install.".into());
        }

        Ok("NOMINAL".into())
    }

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

    pub async fn diagnose_workspace() -> anyhow::Result<String> {
        let backend = Self::audit_backend().await.unwrap_or_else(|e| e.to_string());
        let ui = Self::audit_ui().await.unwrap_or_else(|e| e.to_string());

        if backend == "NOMINAL" && ui == "NOMINAL" {
            return Ok("WORKSPACE_NOMINAL: Integrity verified across Rust and Svelte domains.".to_string());
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

    pub fn probe_shadow_activity() -> Vec<String> {
        let mut alerts = Vec::new();
        let targets = ["./src", "./ui/src", "./zig"];

        for path in targets {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    let now = SystemTime::now();
                    if let Ok(elapsed) = now.duration_since(modified) {
                        if elapsed.as_secs() < 60 {
                            alerts.push(format!("SHADOW_ACTIVITY: {} modified recently ({}s ago)", path, elapsed.as_secs()));
                        }
                    }
                }
            }
        }
        alerts
    }
}
