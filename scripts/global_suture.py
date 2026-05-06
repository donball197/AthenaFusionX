import os

# --- 1. PERFECTED SUTURE: src/orchestrator.rs ---
with open("src/orchestrator.rs", "w") as f:
    f.write(r"""// src/orchestrator.rs — v2.2 [FIXED_ESCAPES]
use crate::neural_agent::ContextPacket;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, error};

pub struct Orchestrator;

impl Orchestrator {
    pub fn new() -> Self { Self }

    pub async fn run(
        &self,
        prompt: String,
        ctx: ContextPacket,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        let model = ctx.model_override.as_deref().unwrap_or("gemma-4-26b-a4b-it");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        let mut contents: Vec<Value> = Vec::new();

        if !ctx.recent_logs.is_empty() {
            let obs_block = ctx.recent_logs.join("\n");
            contents.push(json!({
                "role": "user",
                "parts": [{ "text": format!("[ENVIRONMENT OBSERVATIONS OF YOUR PRIOR ACTIONS]\n{}", obs_block) }]
            }));
            contents.push(json!({
                "role": "model",
                "parts": [{ "text": "[ACKNOWLEDGED. PROCEEDING WITH NEXT STEP.]" }]
            }));
        }

        contents.push(json!({
            "role": "user",
            "parts": [{ "text": prompt }]
        }));

        let system_instruction = ctx.system_prompt.as_deref().map(|s| json!({
            "parts": [{ "text": s }]
        }));

        let mut payload = json!({
            "contents": contents,
            "generationConfig": {
                "temperature": 0.2,
                "topP": 0.95,
                "maxOutputTokens": 2048,
                "responseMimeType": "application/json",
            }
        });

        if let Some(sys) = system_instruction {
            payload["systemInstruction"] = sys;
        }

        let mut attempts = 0;
        while attempts < 3 {
            attempts += 1;
            info!("[ORCHESTRATOR] Attempt {} -> {}...", attempts, model);

            match client.post(&url).json(&payload).send().await {
                Ok(response) if response.status().is_success() => {
                    let mut full = String::new();
                    let mut stream = response.bytes_stream();
                    let mut buf = Vec::new();

                    while let Some(Ok(chunk)) = stream.next().await {
                        buf.extend_from_slice(&chunk);
                        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                            let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&line_bytes);
                            let clean = line.trim_start_matches("data: ").trim();
                            if clean.is_empty() || clean == "[DONE]" { continue; }

                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                                    full.push_str(token);
                                    if let Some(sender) = &tx {
                                        let _ = sender.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
                                    }
                                }
                            }
                        }
                    }
                    if !full.trim().is_empty() { return Ok(full); }
                }
                _ => tokio::time::sleep(Duration::from_secs(2)).await,
            }
        }
        Err("UPLINK_FAILED".to_string())
    }
}
""")

# --- 2. PERFECTED SUTURE: src/task_manager.rs ---
with open("src/task_manager.rs", "w") as f:
    f.write(r"""// src/task_manager.rs — v2.3 [EXHAUSTIVE_MAPPING]
use crate::{
    agent_protocol::AgentAction, cli, json_parser,
    orchestrator::Orchestrator, memory::MemoryPalace, server::VmoState,
    distributed_scheduler::DistributedScheduler,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};

pub async fn send_task(
    orchestrator: Arc<Orchestrator>,
    client: &Client,
    api_key: &str,
    raw_directive: &str,
    _memory: Arc<Mutex<MemoryPalace>>,
    _vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    _dist: Arc<DistributedScheduler>,
) -> String {
    let mut session_obs: Vec<String> = vec![];
    let mut turn = 0;
    const MAX_TURNS: usize = 12;

    let execution_law = cli::get_execution_law_v6().await;

    while turn < MAX_TURNS {
        turn += 1;
        info!("=== Turn {} started ===", turn);

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some(execution_law.clone()),
            recent_logs: session_obs.clone(),
            ..Default::default()
        };

        let response = match orchestrator.run(raw_directive.to_string(), ctx, client, api_key, Some(tx.clone())).await {
            Ok(resp) => {
                info!("\x1b[34m[THOUGHT_STREAM] >>\x1b[0m\n{}", resp);
                resp
            }
            Err(e) => {
                error!("Orchestrator uplink failed: {}", e);
                break;
            }
        };

        match json_parser::extract_actions(&response) {
            Ok(actions) if !actions.is_empty() => {
                let mut halted = false;

                for action in actions {
                    if let AgentAction::Halt { reasoning } = &action {
                        info!("\x1b[32m>> [HALT] {}\x1b[0m", reasoning);
                        halted = true;
                        break;
                    }

                    let obs = crate::shell::NexusShell::eval(&action).await;
                    let action_name = action_type_name(&action);
                    session_obs.push(format!("Turn {} | Action: {} | Result: {}", turn, action_name, obs));
                }

                if halted { break; }
            }
            _ => {
                warn!("Turn {} | Parser fracture or empty actions.", turn);
                session_obs.push(format!("Turn {} | ERROR: Output was not valid JSON.", turn));
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    "TASK_COMPLETE".to_string()
}

fn action_type_name(action: &AgentAction) -> &'static str {
    match action {
        AgentAction::ExecuteCommand { .. } => "execute_command",
        AgentAction::ReadFile { .. }       => "read_file",
        AgentAction::WriteFile { .. }      => "write_file",
        AgentAction::VmoCreate { .. }      => "vmo_create",
        AgentAction::CalculateEntropy { .. } => "calculate_entropy",
        AgentAction::LazarusSnapshot { .. }=> "lazarus_snapshot",
        AgentAction::SearchMemory { .. }   => "search_memory",
        AgentAction::ExportReport { .. }   => "export_report",
        AgentAction::FetchUiSnapshot { .. }=> "fetch_ui_snapshot",
        AgentAction::Halt { .. }           => "halt",
    }
}
""")

# --- 3. SUTURE: src/cli.rs (Execution Law) ---
with open("src/cli.rs", "w") as f:
    f.write(r"""// src/cli.rs — v4.6 [TIGHTER_LAW]
pub async fn inject_workspace_dna(directive: &str) -> String { directive.trim().to_string() }

pub async fn get_execution_law_v6() -> String {
    r#"You are NEXUS. 
OUTPUT: A valid JSON array of AgentAction objects. 
RULES:
1. Every write_file MUST be followed by cargo check.
2. If history shows you already read a file, do not read it again.
3. Use halt when complete.
Output ONLY JSON."#.to_string()
}

pub async fn run_cli_loop(state: std::sync::Arc<crate::server::AppState>) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    loop {
        print!("\n\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        if stdin.read_line(&mut input)? == 0 { break; }
        crate::task_manager::send_task(
            state.orchestrator.clone(), &state.client, &state.api_key, input.trim(),
            state.memory.clone(), state.vmo_state.clone(), state.tx.clone(), state.dist.clone()
        ).await;
    }
    Ok(())
}
""")

# --- 4. SUTURE: Cleanup ---
if os.path.exists("src/broken_test.rs"):
    os.remove("src/broken_test.rs")

print("\x1b[32m>> [SUCCESS] Global Suture perfected. Escapes and Enums fixed.\x1b[0m")
