#!/usr/bin/env bash
set -euo pipefail

echo ">> [NEXUS] Commencing Full Architecture Consolidation (v3.8.2)..."

# 1. CORE PROTOCOL
cat << 'INNER_EOF' > src/agent_protocol.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AgentStatus { Idle, Executing, Fault(String) }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AgentTaskType { Refactor, Audit, SystemCheck, NeuralInference, Command, DevOps }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentDirective {
    pub id: Uuid,
    pub task_type: AgentTaskType,
    pub payload: String,
    pub priority: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum AgentAction {
    #[serde(rename = "execute_command")] ExecuteCommand { command: String, reasoning: String },
    #[serde(rename = "read_file")] ReadFile { path: String, reasoning: String },
    #[serde(rename = "write_file")] WriteFile { path: String, content: String, reasoning: String },
    #[serde(rename = "search_memory")] SearchMemory { query: String, reasoning: String },
    #[serde(rename = "vmo_create")] VmoCreate { size: usize, reasoning: String },
    #[serde(rename = "calculate_entropy")] CalculateEntropy { handle: String, reasoning: String },
    #[serde(rename = "lazarus_snapshot")] LazarusSnapshot { path: String, tag: Option<String>, reasoning: String },
    #[serde(rename = "export_report")] ExportReport { title: String, content: String, reasoning: String },
    #[serde(rename = "fetch_ui_snapshot")] FetchUiSnapshot { reasoning: String },
    #[serde(rename = "halt")] Halt { reasoning: String },
}
INNER_EOF

# 2. THE GREEDY PARSER (Repetition Resistant)
cat << 'INNER_EOF' > src/json_parser.rs
use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use regex::Regex;
use once_cell::sync::Lazy;

static ARRAY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\s*\{[\s\S]*?\}\s*\]").unwrap());

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut text = response.trim().to_string();
    text = text.replace("```json", "").replace("```", "").replace("\n", " ").trim().to_string();

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
    Err(anyhow!("PARSER_FRACTURE: No valid action array found."))
}
INNER_EOF

# 3. THE HYBRID ORCHESTRATOR (Nucleus Sampling Enabled)
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
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(240))
            .build().map_err(|e| e.to_string())?;

        let model = ctx.model_override.as_deref().unwrap_or("gemma-4-26b-a4b-it");
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": 0.55,
                "topP": 0.90,
                "topK": 40,
                "maxOutputTokens": 2048,
                "responseMimeType": "application/json"
            }
        });

        let mut attempts = 0;
        while attempts < 3 {
            attempts += 1;
            info!("[ORCHESTRATOR] Attempt {} → {}...", attempts, model);
            let res = client.post(&url).json(&payload).send().await;

            match res {
                Ok(response) if response.status().is_success() => {
                    let mut full = String::new();
                    let mut stream = response.bytes_stream();
                    let mut buf = Vec::new();
                    while let Some(Ok(chunk)) = stream.next().await {
                        buf.extend_from_slice(&chunk);
                        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                            let line = String::from_utf8_lossy(&buf.drain(..=pos).collect::<Vec<_>>());
                            let clean = line.trim_start_matches("data: ").trim();
                            if clean.is_empty() || clean == "[DONE]" { continue; }
                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                                    full.push_str(token);
                                    if let Some(s) = &tx { let _ = s.send(json!({"type":"NEURAL_STREAM","payload":token}).to_string()); }
                                }
                            }
                        }
                    }
                    return Ok(full);
                }
                Ok(r) => error!("[ORCH] API Error {}: {}", r.status(), r.text().await.unwrap_or_default()),
                Err(e) => error!("[ORCH] Transport Error: {}", e),
            }
            tokio::time::sleep(Duration::from_millis(1500 * attempts as u64)).await;
        }
        Err("UPLINK_TIMEOUT".into())
    }
}
INNER_EOF

# 4. EXECUTION LAW V6 (Psychological Armor)
cat << 'INNER_EOF' > src/cli.rs
use crate::task_manager;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::process::Command;

pub async fn inject_workspace_dna(directive: &str) -> String {
    format!(r#"[NEXUS EXECUTION LAW v6]
MANDATORY: Output ONLY a valid JSON array. No explanations.
Rules:
- If you start repeating, immediately output ] and stop.
- Never repeat a phrase more than twice.
- Every write_file MUST be followed by "cargo check".
- Max 5 actions.

DIRECTIVE:
{}"#, directive)
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    loop {
        print!("\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        io::stdout().flush().ok();
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let d = inject_workspace_dna(input.trim()).await;
        task_manager::send_task(state.orchestrator.clone(), &state.client, &state.api_key, &d, state.memory.clone(), state.vmo_state.clone(), state.tx.clone(), state.dist.clone()).await;
    }
}
INNER_EOF

# 5. TRI-STATE SHIELD
cat << 'INNER_EOF' > src/podman.rs
use std::process::{Command, Output};
use std::io::{Error, ErrorKind, Result};

pub struct PodmanShield;
impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        let cmd = command.to_lowercase();
        if cmd.contains("rm -rf src") || cmd.contains("rm -rf .git") {
            return Err(Error::new(ErrorKind::PermissionDenied, "AXIOM_BLOCK: Destructive path."));
        }
        if cmd.starts_with("cargo ") || cmd.starts_with("ls") || cmd.starts_with("cat") {
            return Command::new("sh").arg("-c").arg(command).output();
        }
        Command::new("podman").args(["run","--rm","--network","none","-v",".:/workspace","-w","/workspace","alpine:latest","sh","-c",command]).output()
    }
}
INNER_EOF

# 6. UI NEURAL GATE (auth.svelte.ts)
cat << 'INNER_EOF' > ui/src/lib/stores/auth.svelte.ts
export const auth = $state({ token: null as string | null, isAuthenticated: false, isInitialized: false });
export async function bootAuth() {
    try {
        const res = await fetch('/api/auth/bootstrap');
        if (res.ok) {
            const data = await res.json();
            auth.token = data.token;
            auth.isAuthenticated = true;
        }
    } finally { auth.isInitialized = true; }
}
INNER_EOF

# 7. UI LAYOUT GATE (+layout.svelte)
cat << 'INNER_EOF' > ui/src/routes/+layout.svelte
<script>
    import { auth, bootAuth } from '$lib/stores/auth.svelte';
    import { onMount } from 'svelte';
    let { children } = $props();
    onMount(() => bootAuth());
</script>

{#if auth.isInitialized}
    {@render children()}
{:else}
    <div style="height:100vh;width:100vw;background:black;color:#00ffff;display:flex;align-items:center;justify-content:center;font-family:monospace;">
        NEURAL HANDSHAKE IN PROGRESS...
    </div>
{/if}
INNER_EOF

# 8. THE SERVER (Axum 0.7 Order Fix)
sed -i 's/app\.into_make_service_with_connect_info::<SocketAddr>().with_state(state)/app.with_state(state).into_make_service_with_connect_info::<SocketAddr>()/' src/server.rs

echo ">> [COMPLETE] Substrate Hardened. Run 'cargo build -j 1' and 'npm run build' in ui/."
