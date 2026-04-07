// NEXUS SINGULARITY — Mission Control v1.9.5
use crate::inquisitor::Inquisitor;
use crate::neural_agent::MicroAgent;
use crate::arbiter_agent::ArbiterAgent;
use crate::lazarus_vault::LazarusVault; // SUTURE: Restored
use crate::backend_autofix::fix_backend; // SUTURE: Restored
use crate::frontend_autofix::fix_ui; // SUTURE: Restored
use crate::server::AppState;
use std::sync::Arc;
use reqwest::Client;
use std::process::Command; // SUTURE: Restored

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

        let prompt = format!("CRITICAL ERROR:\n{}\n\nRETURN JSON ONLY: {{\"file\": \"path\", \"fix\": \"code\"}}", error_log);
        
        if let Ok((response, _)) = agent.prepare_and_run(&client, api_key, &prompt, Default::default(), 0.2).await {
            let start = response.find('{');
            let end = response.rfind('}');
            
            if let (Some(s), Some(e)) = (start, end) {
                let clean_json = &response[s..=e];
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(clean_json) {
                    let file_path = json["file"].as_str().unwrap_or_default();
                    let fix_code = json["fix"].as_str().unwrap_or_default();

                    if !file_path.is_empty() && !fix_code.is_empty() {
                        let arbiter = ArbiterAgent::new(agent.clone(), None);
                        if let Ok(report) = arbiter.analyze_directive(api_key, &format!("Audit patch for {}: {}", file_path, fix_code)).await {
                            if report.can_proceed && report.risk_score < 0.45 {
                                let _ = std::fs::write(file_path, fix_code);
                                println!("\x1b[32m>> [SURGERY] Patch applied to {}\x1b[0m", file_path);
                            }
                        }
                    }
                }
            }
        }
    }
}

// SUTURE: The logic previously swallowed by the EOF comment
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
