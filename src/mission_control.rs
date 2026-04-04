// NEXUS SINGULARITY — Mission Control v1.9.5
// Upgrade: Integrated ArbiterAgent as safety gate before applying patches

use crate::inquisitor::Inquisitor;
use crate::neural_agent::MicroAgent;
use crate::lazarus_vault::LazarusVault;
use crate::backend_autofix::fix_backend;
use crate::frontend_autofix::fix_ui;
use crate::server::AppState;
use crate::arbiter_agent::ArbiterAgent;
use std::sync::Arc;
use reqwest::Client;
use std::process::Command;

const CURRENT_VERSION: &str = "v1.9.5";

pub async fn run_repair_sequence(agent: Arc<MicroAgent>, api_key: &str, target: &str) {
    let client = Client::new();
    println!("\x1b[35m>> [MISSION CONTROL] Diagnosis: {}\x1b[0m", target);

    for _attempt in 1..=3 {
        let error_log = if target == "ui" {
            Inquisitor::audit_ui().unwrap_or_else(|e| e.to_string())
        } else {
            Inquisitor::audit_backend().unwrap_or_else(|e| e.to_string())
        };

        if error_log == "NOMINAL" {
            println!("\x1b[32m>> [STABLE] {} nominal\x1b[0m", target);
            return;
        }

        let prompt = format!(
            "CRITICAL ERROR:\n{}\n\nRETURN JSON: {{\"file\": \"path\", \"fix\": \"code\"}}",
            error_log
        );

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some("NEXUS SURGEON. ONLY JSON with file and fix.".into()),
            ..Default::default()
        };

        if let Ok((response, _)) = agent.prepare_and_run(&client, api_key, &prompt, ctx, 0.2).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                let file_path = json["file"].as_str().unwrap_or("").to_string();
                let fix_code = json["fix"].as_str().unwrap_or("").to_string();

                if !file_path.is_empty() && !fix_code.trim().is_empty() {
                    println!("\x1b[33m>> [SURGERY] Proposed patch for {}\x1b[0m", file_path);

                    // === NEW: ARBETER SAFETY GATE ===
                    let arbiter = ArbiterAgent::new(agent.clone(), None);
                    match arbiter.analyze_directive(api_key, &format!(
                        "Evaluate this proposed code patch for file '{}':\n\n{}", 
                        file_path, fix_code
                    )).await {
                        Ok(report) if report.can_proceed && report.risk_score < 0.45 => {
                            println!("\x1b[32m>> [ARBITER APPROVED] Risk score: {:.2} — Applying patch...\x1b[0m", report.risk_score);
                            
                            match std::fs::write(&file_path, &fix_code) {
                                Ok(_) => println!("\x1b[32m>> [SURGERY] Patch applied successfully to {}\x1b[0m", file_path),
                                Err(e) => println!("\x1b[31m!! [SURGERY] Write failed: {}\x1b[0m", e),
                            }
                        }
                        Ok(report) => {
                            println!("\x1b[31m!! [ARBITER REJECTED] Risk score: {:.2} — Reason: {}\x1b[0m", 
                                report.risk_score, report.recommendation);
                        }
                        Err(e) => {
                            println!("\x1b[33m!! [ARBITER ERROR] {}\x1b[0m — Falling back to manual review.", e);
                        }
                    }
                }
            }
        }
    }
}

pub async fn perform_system_upgrade(state: Arc<AppState>) -> String {
    println!("\x1b[36m>> [UPGRADE] v{}\x1b[0m", CURRENT_VERSION);
    let _ = LazarusVault::auto_snapshot("src/main.rs", "pre-upgrade");
    let _ = fix_backend(false, &state.agent, &state.client, &state.api_key).await;
    let _ = fix_ui(false, &state.agent, &state.client, &state.api_key).await;

    let backend_ok = Command::new("cargo").args(["build"]).status().map(|s| s.success()).unwrap_or(false);
    let ui_ok = Command::new("npm").current_dir("ui").args(["run", "build"]).status().map(|s| s.success()).unwrap_or(false);

    format!(">> [UPGRADE] Complete. Backend: {} | UI: {}", backend_ok, ui_ok)
}

pub async fn upgrade_command(state: Arc<AppState>) -> String {
    perform_system_upgrade(state).await
}
