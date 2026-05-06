#!/usr/bin/env bash
set -euo pipefail

echo ">> [NEXUS] Fusing Upgraded Cortex DNA (v3.8.5)..."

# 1. TASK MANAGER (Gemma 3 27B-it API Enforced & Inline Check)
cat << 'INNER_EOF' > src/task_manager.rs
use anyhow::{anyhow, Result};
use crate::{
    agent_protocol::AgentAction, cli, json_parser, neural_agent::MicroAgent,
    orchestrator::Orchestrator, memory::MemoryPalace, server::VmoState,
    distributed_scheduler::DistributedScheduler, diff::DiffEngine,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};
use std::path::Path;
use tokio::fs;

pub async fn send_task(
    orchestrator: Arc<Orchestrator>, client: &Client, api_key: &str, raw_directive: &str,
    memory: Arc<Mutex<MemoryPalace>>, vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>, dist: Arc<DistributedScheduler>,
) -> String {
    let mut session_obs = vec![];
    info!("Starting task with directive length: {}", raw_directive.len());

    let mut turn = 0;
    const MAX_TURNS: usize = 8;
    let directive = cli::inject_workspace_dna(raw_directive).await;

    while turn < MAX_TURNS {
        turn += 1;
        info!("=== Turn {} started ===", turn);

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some("You are NEXUS, a precise autonomous coding agent. Follow the execution law strictly.".to_string()),
            // STRICT GEMMA 3 27B-IT API ENFORCEMENT
            model_override: Some("gemma-3-27b-it".to_string()), 
            ..Default::default()
        };

        let result = orchestrator.run(directive.clone(), ctx, client, api_key, Some(tx.clone())).await;

        let response = match result {
            Ok(resp) => resp,
            Err(e) => {
                error!("Orchestrator uplink failed: {}", e);
                session_obs.push(format!("UPLINK_ERROR: {}", e));
                break;
            }
        };

        match json_parser::extract_actions(&response) {
            Ok(actions) if !actions.is_empty() => {
                info!("Successfully parsed {} actions", actions.len());
                for (i, action) in actions.iter().enumerate() {
                    info!("Executing action {}/{}: {:?}", i + 1, actions.len(), action);
                    let outcome = execute_single_action(action, &memory, &vmo_state, &dist).await;
                    session_obs.push(format!("ACTION_{}: {}", i, outcome));

                    if let AgentAction::WriteFile { .. } = action {
                        info!("Enforcing mandatory 'cargo check' after write_file");
                        let check_action = AgentAction::ExecuteCommand {
                            command: "cargo check".to_string(),
                            reasoning: "Mandatory verification after file modification".to_string(),
                        };
                        let _ = execute_single_action(&check_action, &memory, &vmo_state, &dist).await;
                    }
                }
                if actions.len() <= 3 { break; }
            }
            Ok(_) => session_obs.push("EMPTY_ACTIONS".to_string()),
            Err(e) => {
                warn!("Parser failure: {}", e);
                session_obs.push(format!("PARSER_FRACTURE: {}", e));
                if turn >= MAX_TURNS - 2 { break; }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
    }
    
    if session_obs.iter().any(|o| o.contains("PARSER_FRACTURE") || o.contains("UPLINK_ERROR")) {
        "TASK_FAULTED".to_string()
    } else {
        "TASK_COMPLETE".to_string()
    }
}

async fn execute_single_action(
    action: &AgentAction, _memory: &Arc<Mutex<MemoryPalace>>, _vmo_state: &Arc<Mutex<VmoState>>, _dist: &Arc<DistributedScheduler>,
) -> String {
    match action {
        AgentAction::ExecuteCommand { command, reasoning } => {
            info!("[EXEC_COMMAND] {} | reasoning: {}", command, reasoning);
            if let Err(e) = crate::arbiter_agent::ArbiterAgent::validate_action("execute_command", command) {
                return format!("BLOCKED: {}", e);
            }
            match tokio::process::Command::new("sh").arg("-c").arg(command).output().await {
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        format!("SUCCESS: {}", String::from_utf8_lossy(&output.stdout).trim())
                    } else {
                        format!("FAILED: {}", stderr.trim())
                    }
                }
                Err(e) => format!("EXEC_ERROR: {}", e),
            }
        }
        AgentAction::WriteFile { path, content, reasoning } => {
            info!("[WRITE_FILE] {} | reasoning: {}", path, reasoning);
            let path = Path::new(path);
            if let Err(e) = crate::sandbox::normalize_and_check(path.to_string_lossy().as_ref()) {
                return format!("WRITE_BLOCKED: {}", e);
            }
            if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent).await; }
            match fs::write(path, content).await {
                Ok(_) => format!("WROTE: {}", path.display()),
                Err(e) => format!("WRITE_ERROR: {}", e),
            }
        }
        AgentAction::ReadFile { path, reasoning } => {
            info!("[READ_FILE] {} | reasoning: {}", path, reasoning);
            match fs::read_to_string(path).await {
                Ok(content) => format!("READ: {} ({} bytes)", path, content.len()),
                Err(e) => format!("READ_ERROR: {}", e),
            }
        }
        _ => "UNSUPPORTED_ACTION".to_string(),
    }
}
INNER_EOF

# 2. CLI (V6.3 Strict JSON Mode + Empty Input Guard)
cat << 'INNER_EOF' > src/cli.rs
use crate::task_manager;
use std::io::{self, Write};
use std::sync::Arc;

pub async fn inject_workspace_dna(directive: &str) -> String {
    let directive = directive.trim();
    if directive.is_empty() { return "[NEXUS] No directive provided.".to_string(); }

    format!(r#"[NEXUS EXECUTION LAW v6.3 — STRICT JSON MODE]
You are NEXUS, a precise autonomous coding agent.

CRITICAL RULES — FOLLOW EXACTLY OR TASK FAILS:
- Output **ONLY** a valid JSON array of AgentAction objects. Nothing else.
- No explanations, no markdown, no ```json, no extra text.
- If unsure or no actions needed, output exactly: []
- Every `write_file` MUST be followed in the same array by an `execute_command` with "cargo check".
- Maximum 5 actions per response.
- Do not repeat any phrase.

DIRECTIVE:
{}

Use these exact action schemas:
- {{"type": "execute_command", "command": "...", "reasoning": "..."}}
- {{"type": "write_file", "path": "...", "content": "...", "reasoning": "..."}}
- {{"type": "read_file", "path": "...", "reasoning": "..."}}

Output ONLY the JSON array."#, directive)
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    loop {
        print!("\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        io::stdout().flush().ok();
        let mut input = String::new();
        if stdin.read_line(&mut input)? == 0 { break; }
        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }
        let d = inject_workspace_dna(trimmed).await;
        task_manager::send_task(state.orchestrator.clone(), &state.client, &state.api_key, &d, state.memory.clone(), state.vmo_state.clone(), state.tx.clone(), state.dist.clone()).await;
    }
    Ok(())
}
INNER_EOF

# 3. ORCHESTRATOR (Safe Lifetimes, Low Temp, No Mime Type)
cat << 'INNER_EOF' > src/orchestrator.rs
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

    pub async fn run(&self, prompt: String, ctx: ContextPacket, _: &Client, api_key: &str, tx: Option<broadcast::Sender<String>>) -> Result<String, String> {
        let client = Client::builder().connect_timeout(Duration::from_secs(30)).timeout(Duration::from_secs(300)).build().map_err(|e| e.to_string())?;
        let model = ctx.model_override.as_deref().unwrap_or("gemma-3-27b-it");
        let url = format!("[https://generativelanguage.googleapis.com/v1beta/models/](https://generativelanguage.googleapis.com/v1beta/models/){}:streamGenerateContent?alt=sse&key={}", model, api_key);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": 0.25,
                "topP": 0.95,
                "topK": 40,
                "maxOutputTokens": 2048,
            }
        });

        let mut attempts = 0;
        while attempts < 3 {
            attempts += 1;
            info!("[ORCHESTRATOR] Attempt {} → {}...", attempts, model);

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
                Ok(r) => error!("[ORCH] API Error {}: {}", r.status(), r.text().await.unwrap_or_default()),
                Err(e) => error!("[ORCH] Transport Error: {}", e),
            }
            tokio::time::sleep(Duration::from_secs(2 * attempts as u64)).await;
        }
        Err("UPLINK_FAILED after 3 attempts".to_string())
    }
}
INNER_EOF

# 4. JSON PARSER (Multi-Strategy Extraction)
cat << 'INNER_EOF' > src/json_parser.rs
use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use regex::Regex;
use once_cell::sync::Lazy;

static ARRAY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"\[\s*(\{[\s\S]*?\}\s*,?\s*)+\]"#).unwrap());

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut text = response.trim().to_string();
    text = text.replace("```json", "").replace("```", "").replace("\n", " ").replace("  ", " ").trim().to_string();

    if let Some(m) = ARRAY_RE.find(&text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(m.as_str()) {
            if !actions.is_empty() { return Ok(actions); }
        }
    }
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        let candidate = &text[start..=end];
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(candidate) {
            if !actions.is_empty() { return Ok(actions); }
        }
    }
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&text) {
        if !actions.is_empty() { return Ok(actions); }
    }
    
    let repaired = repair_json_response(&text);
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&repaired) {
        if !actions.is_empty() { return Ok(actions); }
    }
    Err(anyhow!("PARSER_FRACTURE: No valid action array found."))
}

fn repair_json_response(text: &str) -> String {
    let mut repaired = text.to_string();
    if !repaired.starts_with('[') {
        if let Some(pos) = repaired.find('[') { repaired = repaired[pos..].to_string(); } 
        else { repaired = format!("[{}]", repaired); }
    }
    if !repaired.ends_with(']') { repaired.push(']'); }
    repaired.replace(",]", "]")
}
INNER_EOF

echo ">> [COMPLETE] Cortex upgrade successfully applied."
