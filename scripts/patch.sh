#!/usr/bin/env bash
# =============================================================================
# NEXUS SINGULARITY — GEMMA 4 MoE CORTEX UPGRADE
# Enforces Gemma 4 MoE Routing, Lexical Path Guards, and Strict Ownership
# =============================================================================
set -euo pipefail

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RESET='\033[0m'

log()  { echo -e "${CYAN}>> [NEXUS_UPGRADE]${RESET} $1"; }
ok()   { echo -e "${GREEN}[OK]${RESET} $1"; }

log "IGNITING NEXUS CORTEX UPGRADE (Gemma 4 26B/31B MoE Baseline)..."
echo ""

# =============================================================================
# 1. src/json_parser.rs — AST & Alias Healing
# =============================================================================
cat << 'EOF' > src/json_parser.rs
use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use serde_json::{json, Value};

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let stripped = response.trim()
        .replace("```json", "")
        .replace("```", "");

    let renamed = stripped
        .replace("\"action\":", "\"type\":")
        .replace("\"action\" :", "\"type\":")
        .replace("\"file_path\":", "\"path\":")
        .replace("\"fn_name\":", "\"function_name\":");

    let start = renamed.find('[')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array start '[' found"))?;
    let end = renamed.rfind(']')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array end ']' found"))?;

    if start >= end {
        return Err(anyhow!("PARSER_FRACTURE: Invalid JSON array bounds"));
    }

    let json_text = &renamed[start..=end];

    if let Ok(mut parsed_json) = serde_json::from_str::<Value>(json_text) {
        if let Some(arr) = parsed_json.as_array_mut() {
            for obj in arr.iter_mut() {
                if let Some(map) = obj.as_object_mut() {
                    if !map.contains_key("type") {
                        if map.contains_key("search") && map.contains_key("replace") {
                            map.insert("type".to_string(), json!("diff_patch"));
                        } else if map.contains_key("command") {
                            map.insert("type".to_string(), json!("execute_command"));
                        } else if map.contains_key("function_name") && map.contains_key("new_body") {
                            map.insert("type".to_string(), json!("patch_function"));
                        } else if map.contains_key("path") && map.contains_key("content") {
                            map.insert("type".to_string(), json!("write_file"));
                        } else if map.contains_key("path") {
                            map.insert("type".to_string(), json!("read_file"));
                        }
                    }
                }
            }
        }

        if let Ok(actions) = serde_json::from_value::<Vec<AgentAction>>(parsed_json) {
            if !actions.is_empty() { return Ok(actions); }
        }
    }

    Err(anyhow!("PARSER_FRACTURE: Malformed or unrecoverable array"))
}
EOF
ok "src/json_parser.rs"

# =============================================================================
# 2. src/logic_engine.rs — Frequency Deadlock Fix
# =============================================================================
cat << 'EOF' > src/logic_engine.rs
use std::collections::HashMap;
use tracing::warn;

pub struct LogicEngine;

impl LogicEngine {
    pub fn is_action_safe(directive: &str) -> bool {
        let forbidden = [
            "rm -rf /", "mkfs", "shutdown", "chmod 777",
            "dd if=/dev/zero", "> /dev/sda", "fork bomb", ":(){ :|:& };:"
        ];
        let d_lower = directive.to_lowercase();
        !forbidden.iter().any(|&p| d_lower.contains(p))
    }

    pub fn detect_deadlock(observations: &[String]) -> bool {
        if observations.len() < 3 { return false; }

        let mut read_counts: HashMap<String, usize> = HashMap::new();
        let mut progress = false;

        for obs in observations {
            if obs.contains("[WRITE_OK]") || obs.contains("[DIFF_PATCH_OK]") || obs.contains("[PATCH_OK]") {
                progress = true;
            }
            if obs.contains("[READ_OK]") {
                if let Some(after) = obs.strip_prefix("[READ_OK] ") {
                    let path = after.split_whitespace().next().unwrap_or("").to_string();
                    if !path.is_empty() {
                        *read_counts.entry(path).or_insert(0) += 1;
                    }
                }
            }
        }

        if !progress && read_counts.values().any(|&count| count >= 3) {
            warn!("[LOGIC] Deadlock: Same file read 3+ times with zero progress.");
            return true;
        }

        let mut last_cmd_sig = String::new();
        let mut cmd_repeat_count = 0;

        for obs in observations {
            if let Some(start) = obs.find("[CMD:") {
                let sig = obs[start..].to_string();
                if sig == last_cmd_sig {
                    cmd_repeat_count += 1;
                } else {
                    last_cmd_sig = sig;
                    cmd_repeat_count = 1;
                }
            }
        }

        if cmd_repeat_count >= 2 {
            warn!("[LOGIC] Deadlock: Command repetition detected.");
            return true;
        }
        false
    }
}
EOF
ok "src/logic_engine.rs"

# =============================================================================
# 3. src/orchestrator.rs — Gemma 4 MoE Integration
# =============================================================================
cat << 'EOF' > src/orchestrator.rs
use crate::neural_agent::ContextPacket;
use crate::metrics::METRICS;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{info, warn, error};

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
            let log_blob = ctx.recent_logs.join("\n");
            contents.push(json!({
                "role": "user",
                "parts": [{ "text": format!("[SYSTEM_OBSERVATIONS]\n{}", log_blob) }]
            }));
            contents.push(json!({
                "role": "model",
                "parts": [{ "text": "[ACKNOWLEDGED]" }]
            }));
        }
        contents.push(json!({ "role": "user", "parts": [{ "text": prompt }] }));

        let payload = json!({
            "contents": contents,
            "systemInstruction": ctx.system_prompt.as_deref()
                .map(|s| json!({"parts": [{"text": s}]})),
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 4096,
                "responseMimeType": "application/json"
            }
        });

        for attempt in 1..=3u64 {
            info!("[ORCHESTRATOR] Uplink Attempt {} → {}", attempt, model);
            METRICS.api_calls.fetch_add(1, Ordering::Relaxed);

            let t_start = Instant::now();
            let res = client.post(&url).timeout(Duration::from_secs(45)).json(&payload).send().await;

            match res {
                Ok(response) if response.status().is_success() => {
                    let mut full_text = String::new();
                    let mut stream = response.bytes_stream();
                    let mut buf: Vec<u8> = Vec::new();

                    while let Some(Ok(chunk)) = stream.next().await {
                        buf.extend_from_slice(&chunk);
                        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                            let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&line_bytes);
                            let clean = line.trim_start_matches("data: ").trim();
                            if clean.is_empty() || clean == "[DONE]" { continue; }
                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val
                                    .pointer("/candidates/0/content/parts/0/text")
                                    .and_then(|v| v.as_str())
                                {
                                    full_text.push_str(token);
                                    if let Some(sender) = &tx {
                                        let _ = sender.send(
                                            json!({"type": "NEURAL_STREAM", "payload": token}).to_string()
                                        );
                                    }
                                }
                            }
                        }
                    }

                    let latency_ms = t_start.elapsed().as_millis() as u64;
                    METRICS.api_latency_ms_sum.fetch_add(latency_ms, Ordering::Relaxed);

                    if !full_text.trim().is_empty() {
                        return Ok(full_text.trim().to_string());
                    }
                    warn!("[ORCHESTRATOR] Empty response on attempt {}, retrying...", attempt);
                }
                Ok(response) => {
                    let status = response.status();
                    let err_body = response.text().await.unwrap_or_default();
                    error!("API_REJECTION [{}]: {}", status, err_body);

                    if status == 404 { return Err(format!("STALE_MODEL_STRING: {}", model)); }
                    if status.as_u16() == 429 || err_body.contains("RESOURCE_EXHAUSTED") || err_body.contains("429") {
                        return Err("RESOURCE_EXHAUSTED".into());
                    }
                    if status == 400 || err_body.contains("400") {
                        return Err("CONTEXT_OVERFLOW".into());
                    }
                }
                Err(e) => warn!("Uplink failure: {}", e),
            }
            tokio::time::sleep(Duration::from_secs(attempt * 2)).await;
        }

        METRICS.task_failures.fetch_add(1, Ordering::Relaxed);
        Err("CORTEX_UPLINK_FAILED".into())
    }
}
EOF
ok "src/orchestrator.rs"

# =============================================================================
# 4. src/conductor.rs — Gemma 4 MoE Dynamic Failover
# =============================================================================
cat << 'EOF' > src/conductor.rs
use crate::metrics::METRICS;
use crate::neural_agent::ContextPacket;
use crate::orchestrator::Orchestrator;
use crate::router::AgentProfile;
use nexus_substrate::token_ledger::wait_for_quota;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;

const ESTIMATED_TOKENS: u32 = 2_000;

pub struct Conductor;

impl Conductor {
    pub async fn run_turn(
        profile: &AgentProfile,
        prompt: String,
        turn: usize,
        orch: &Orchestrator,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        METRICS.api_calls.fetch_add(1, Ordering::Relaxed);

        let model: &str = if turn > 6 { "gemma-4-31b-it" } else { "gemma-4-26b-a4b-it" };
        wait_for_quota(model, ESTIMATED_TOKENS).await;

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(
                json!({"type": "NEURAL_THOUGHT", "payload": format!("[CONDUCTOR] turn {} → {}", turn, model)}).to_string()
            );
        }

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

        if let Err(ref e) = result {
            if e.contains("STALE") || e.contains("500") || e.contains("RESOURCE_EXHAUSTED") || e.contains("INTERNAL") || e.contains("CONTEXT_OVERFLOW") {
                METRICS.task_failures.fetch_add(1, Ordering::Relaxed);
                
                let failover: &str = if model == "gemma-4-31b-it" { "gemma-4-26b-a4b-it" } else { "gemma-4-31b-it" };
                wait_for_quota(failover, ESTIMATED_TOKENS).await;

                let fail_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some(failover.to_string()),
                    ..Default::default()
                };
                return orch.run(prompt, fail_ctx, client, api_key, tx).await;
            }
        }
        result
    }
}
EOF
ok "src/conductor.rs"

# =============================================================================
# 5. src/task_manager.rs — UTF8 Safe & Context Suture
# =============================================================================
cat << 'EOF' > src/task_manager.rs
use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction, cli, json_parser, logic_engine::LogicEngine,
    metrics::METRICS, orchestrator::Orchestrator, executor::RoboticExecutor,
    server::VmoState, distributed_scheduler::DistributedScheduler,
};
use reqwest::Client;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};

const FORMAT_CORRECTION: &str = "\
[FORMAT_VIOLATION] Your previous response was REJECTED — it contained prose \
before the JSON array. You MUST output ONLY a raw JSON array. \
Start your response with `[` and end with `]`. \
No reasoning text outside JSON. Retry the same action now.";

fn truncate_obs(obs: String, max_chars: usize) -> String {
    let char_count = obs.chars().count();
    if char_count <= max_chars { return obs; }
    let truncated: String = obs.chars().take(max_chars).collect();
    format!("{}... [TRUNCATED {}/{} chars]", truncated, max_chars, char_count)
}

pub async fn send_task(
    orchestrator: Arc<Orchestrator>,
    client: &Client,
    api_key: &str,
    raw_directive: &str,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) -> String {
    METRICS.inc_task();

    let mut session_obs: Vec<String> = vec![];
    let mut turn = 0usize;
    let mut consecutive_fractures = 0usize;
    let mut mission_outcome = "INCOMPLETE".to_string();

    const MAX_TURNS: usize = 12;
    const MAX_CONSECUTIVE_FRACTURES: usize = 3;
    const OBS_WINDOW: usize = 5;
    const MAX_OBS_CHARS: usize = 2500;

    let execution_law = cli::get_execution_law_v6().await;
    let mut executor = RoboticExecutor::new(api_key.to_string(), tx.clone());

    'mission: while turn < MAX_TURNS {
        turn += 1;
        info!("=== Turn {} / {} ===", turn, MAX_TURNS);

        if LogicEngine::detect_deadlock(&session_obs) {
            warn!("[DEADLOCK] Cortex Loop detected. Aborting mission.");
            METRICS.inc_failure();
            let _ = tx.send(serde_json::json!({"type": "TASK_STATUS", "payload": "DEADLOCK_ABORT"}).to_string());
            break 'mission;
        }

        if consecutive_fractures >= MAX_CONSECUTIVE_FRACTURES {
            warn!("[FRACTURE_STORM] {} consecutive failures. Aborting.", consecutive_fractures);
            METRICS.inc_failure();
            let _ = tx.send(serde_json::json!({"type": "TASK_STATUS", "payload": "FRACTURE_STORM_ABORT"}).to_string());
            break 'mission;
        }

        let window = session_obs.iter().rev().take(OBS_WINDOW).rev().cloned().collect::<Vec<_>>();

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some(execution_law.clone()),
            recent_logs: window,
            ..Default::default()
        };

        let response = match orchestrator.run(raw_directive.to_string(), ctx, client, api_key, Some(tx.clone())).await {
            Ok(resp) => resp,
            Err(ref e) if e.contains("CONTEXT_OVERFLOW") => {
                warn!("[CONTEXT_OVERFLOW] Pruning observation history and retrying...");
                let keep = session_obs.len() / 2;
                session_obs = session_obs.split_off(session_obs.len() - keep);
                continue 'mission;
            }
            Err(e) => {
                error!("Uplink failed: {}", e);
                break 'mission;
            }
        };

        match json_parser::extract_actions(&response) {
            Ok(actions) => {
                if actions.is_empty() { break 'mission; }
                consecutive_fractures = 0;

                match executor.execute_handle(actions.clone(), memory.clone(), vmo_state.clone(), dist.clone()).await {
                    Ok(observations) => {
                        for obs in observations {
                            let clean = truncate_obs(obs, MAX_OBS_CHARS);
                            session_obs.push(clean.clone());
                            if clean.contains("_OK") || clean.contains("[CMD:") { break; }
                        }
                    }
                    Err(e) => session_obs.push(format!("EXECUTOR_FAULT: {}", e)),
                }

                if actions.iter().any(|a| matches!(a, AgentAction::Halt { .. })) {
                    info!(">> [MISSION COMPLETE] Agent halted after {} turns.", turn);
                    mission_outcome = "COMPLETE".to_string();
                    break 'mission;
                }
            }
            Err(e) => {
                consecutive_fractures += 1;
                warn!("Turn {} | Parser fracture ({}/{}): {}", turn, consecutive_fractures, MAX_CONSECUTIVE_FRACTURES, e);
                session_obs.retain(|o| !o.starts_with("[FORMAT_VIOLATION]"));
                session_obs.push(FORMAT_CORRECTION.to_string());
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let obs_summary = session_obs.iter().filter(|o| o.contains("_OK") || o.contains("FAULT") || o.contains("[CMD:")).cloned().collect::<Vec<_>>().join(" | ");
    let mem = memory.lock().await;
    let _ = mem.store_memory("TASK_MANAGER", raw_directive, &format!("[{}] turns={} | {}", mission_outcome, turn, obs_summary), None).await;

    "TASK_COMPLETE".to_string()
}
EOF
ok "src/task_manager.rs"

# =============================================================================
# 6. src/factory.rs — Gemma 4 MoE Baseline
# =============================================================================
cat << 'EOF' > src/factory.rs
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AgentFactory;

impl AgentFactory {
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        if turn >= 8 || current_error_count > 1 { "gemma-4-31b-it" } else { "gemma-4-26b-a4b-it" }
    }

    pub fn select_model_for_mission(mission_type: &str) -> &'static str {
        match mission_type {
            "heavy_reasoning" | "refactor" | "security" | "dream" => "gemma-4-31b-it",
            _ => "gemma-4-26b-a4b-it",
        }
    }

    pub fn get_persona_for_mission(mission_type: &str) -> String {
        match mission_type {
            "security" => "IDENTITY: NEXUS Security Auditor. ROLE: Identify logic flaws, path traversal, unsafe unwraps, and auth gaps. OUTPUT: ONLY a valid JSON array. No prose.".to_string(),
            "docs" => "IDENTITY: NEXUS Archivist. ROLE: Generate technical markdown, README files, and architecture maps. OUTPUT: ONLY a valid JSON array of write_file actions.".to_string(),
            "qa" => "IDENTITY: NEXUS QA Agent. ROLE: Ensure 100% test coverage and zero compiler warnings. WORKFLOW: cargo check → cargo test → fix failures → halt. OUTPUT: ONLY a valid JSON array.".to_string(),
            "dream" => "IDENTITY: NEXUS Archivist. ROLE: Synthesize memory strands into architectural insights. OUTPUT: Plain text summary — no JSON required.".to_string(),
            _ => "IDENTITY: NEXUS Lead DevOps. ROLE: Write high-performance Rust code and manage system operations. OUTPUT: ONLY a valid JSON array. No prose.".to_string(),
        }
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format!("--- NEXUS DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSTAMP: VERIFIED_AGENT_DNA\nTS: {}\n\n{}", client, title, ts, content)
    }
}
EOF
ok "src/factory.rs"

# =============================================================================
# 7. src/server.rs — Lexical Traversal Guard & Substrate Import Fixes
# =============================================================================
cat << 'EOF' > src/server.rs
use nexus_substrate::token_ledger;
use nexus_substrate::memory::MemoryPalace;
use nexus_substrate::vmo_handler;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router, middleware,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tokio::net::TcpListener;

use crate::{
    auth, cluster_api, logic_engine::LogicEngine, security,
    vitals_handler, ws_bridge,
};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client:            reqwest::Client,
    pub monitor:           Arc<tokio::sync::Mutex<crate::system_monitor::SystemMonitor>>,
    pub agent:             Arc<crate::neural_agent::MicroAgent>,
    pub orchestrator:      Arc<crate::orchestrator::Orchestrator>,
    pub memory:            Arc<tokio::sync::Mutex<MemoryPalace>>,
    pub api_key:           String,
    pub current_resonance: Arc<std::sync::atomic::AtomicU32>,
    pub tx:                tokio::sync::broadcast::Sender<String>,
    pub scheduler_tx:      tokio::sync::mpsc::Sender<crate::scheduler::NexusTask>,
    pub arbiter:           Arc<crate::arbiter_agent::ArbiterAgent>,
    pub workspace_root:    PathBuf,
    pub vmo_state:         Arc<tokio::sync::Mutex<crate::server::VmoState>>,
    pub cluster:           Arc<crate::cluster::ClusterMesh>,
    pub dist:              Arc<crate::distributed_scheduler::DistributedScheduler>,
    pub governance_log:    Arc<parking_lot::Mutex<crate::governance::protocol::GovernanceLog>>,
}

#[derive(Default, Debug)]
pub struct VmoState {
    pub last_parent: Option<u32>,
    pub last_child:  Option<u32>,
}

#[derive(serde::Deserialize)]
pub struct Intent {
    pub intent_type: Option<String>,
    pub directive:   Option<String>,
    pub payload:     Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct LoginRequest { key: String }

#[derive(Deserialize)]
struct FsQuery { path: Option<String> }

fn safe_path(workspace_root: &PathBuf, user_path: &str) -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
    if user_path.is_empty() { 
        return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "PATH_EMPTY" })))); 
    }
    
    let path = std::path::Path::new(user_path);
    // Lexical Guard: Reject absolute paths or any directory traversal '..' without invoking filesystem
    if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err((StatusCode::FORBIDDEN, Json(json!({ "error": "PATH_TRAVERSAL_DENIED" }))));
    }
    
    Ok(workspace_root.join(path))
}

pub async fn login_handler(Json(payload): Json<LoginRequest>) -> impl IntoResponse {
    let server_key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    if !server_key.is_empty() && payload.key == server_key {
        match auth::issue_sovereign_token() {
            Ok(token) => (StatusCode::OK, Json(json!({ "token": token }))).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed").into_response(),
        }
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid DNA Signature").into_response()
    }
}

async fn bootstrap_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    if !addr.ip().is_loopback() { return (StatusCode::FORBIDDEN, "Loopback only").into_response(); }
    match std::fs::read_to_string(".nexus_ui_token") {
        Ok(token) => (StatusCode::OK, Json(json!({ "token": token.trim() }))).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Token not yet generated").into_response(),
    }
}

pub async fn unified_gateway(ConnectInfo(addr): ConnectInfo<SocketAddr>, State(state): State<Arc<AppState>>, Json(intent): Json<Intent>) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if let Err(retry_ms) = token_ledger::http_check(&ip) {
        return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, format!("{}", retry_ms / 1000 + 1))], Json(json!({ "status": "RATE_LIMITED", "retry_after_ms": retry_ms }))).into_response();
    }
    let directive_str = intent.directive.or_else(|| intent.payload.as_ref()?.get("directive")?.as_str().map(|s| s.to_string())).unwrap_or_default();
    if !LogicEngine::is_action_safe(&directive_str) { return (StatusCode::FORBIDDEN, Json(json!({ "status": "LOGIC_VIOLATION" }))).into_response(); }
    let s = state.clone();
    let d_clone = directive_str.clone();
    tokio::spawn(async move {
        let d = crate::cli::inject_workspace_dna(&d_clone).await;
        crate::task_manager::send_task(s.orchestrator.clone(), &s.client, &s.api_key, &d, s.memory.clone(), s.vmo_state.clone(), s.tx.clone(), s.dist.clone()).await;
    });
    (StatusCode::OK, Json(json!({ "status": "QUEUED" }))).into_response()
}

async fn fs_read_handler(State(state): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> impl IntoResponse {
    let user_path = q.path.unwrap_or_default();
    match safe_path(&state.workspace_root, &user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => match tokio::fs::read_to_string(&safe).await {
            Ok(content) => Json(json!({ "content": content })).into_response(),
            Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
        },
    }
}

async fn fs_write_handler(State(state): State<Arc<AppState>>, Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let user_path = payload["path"].as_str().unwrap_or_default();
    let content   = payload["content"].as_str().unwrap_or_default();
    match safe_path(&state.workspace_root, user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => {
            if let Some(parent) = safe.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await { return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(); }
            }
            match tokio::fs::write(&safe, content).await {
                Ok(_) => Json(json!({ "status": "success" })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
            }
        }
    }
}

async fn fs_list_handler(State(state): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> impl IntoResponse {
    let user_path = q.path.unwrap_or_else(|| ".".into());
    match safe_path(&state.workspace_root, &user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => {
            let mut entries = Vec::new();
            if let Ok(mut read_dir) = tokio::fs::read_dir(&safe).await {
                while let Ok(Some(entry)) = read_dir.next_entry().await {
                    let meta = entry.metadata().await.ok();
                    entries.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "path": entry.path().strip_prefix(&state.workspace_root).unwrap_or(&entry.path()).to_string_lossy(),
                        "is_dir": meta.map(|m| m.is_dir()).unwrap_or(false)
                    }));
                }
            }
            Json(entries).into_response()
        }
    }
}

async fn metrics_handler() -> impl IntoResponse {
    let metrics_text = crate::metrics::get_metrics_text();
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], metrics_text)
}

pub async fn start_server(state: Arc<AppState>) {
    let api_routes = Router::new()
        .route("/task",   post(unified_gateway))
        .route("/intent", post(unified_gateway))
        .route("/vmos",   get(vmo_handler::handle_get_vmos))
        .route("/vitals", get(vitals_handler::handle_vitals))
        .route("/metrics", get(metrics_handler))
        .route("/cluster/nodes", get(cluster_api::list_nodes))
        .route("/cluster/forge", post(crate::edge_worker::run_remote_synthesis))
        .route("/cluster/run",   post(crate::edge_worker::run_remote_job))
        .route("/fs/read",  get(fs_read_handler))
        .route("/fs/write", post(fs_write_handler))
        .route("/fs/list",  get(fs_list_handler))
        .route("/lazarus/snapshots", get(|| async { Json(crate::lazarus_vault::LazarusVault::list_snapshots()) }))
        .layer(middleware::from_fn(security::sovereign_auth_guard));

    let app = Router::new()
        .route("/api/auth/login",     post(login_handler))
        .route("/api/auth/bootstrap", get(bootstrap_handler))
        .nest("/api", api_routes)
        .route("/ws/telemetry", get(ws_bridge::ws_telemetry_handler))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK BRIDGE LIVE: http://localhost:3000\x1b[0m");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };
    match Assets::get(&final_path) {
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&final_path).first_or_octet_stream().as_ref())], content.data).into_response(),
        None => match Assets::get("index.html") {
            Some(content) => ([(header::CONTENT_TYPE, "text/html")], content.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}
EOF
ok "src/server.rs"

# =============================================================================
# 8. src/main.rs — E0382 Ownership Fix & Clean Envs
# =============================================================================
cat << 'EOF' > src/main.rs
use jsonwebtoken::crypto::Provider;
use nexus_singularity::{
    arbiter_agent::ArbiterAgent,
    cli::run_cli_loop,
    orchestrator::Orchestrator,
    security,
    self_healing_engine::SelfHealingEngine,
    server::{start_server, AppState, VmoState},
};
use nexus_substrate::memory::MemoryPalace;
use std::{env, sync::Arc};
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() {
    Provider::install_default().ok();
    println!(">> [NEXUS] IGNITION SEQUENCE INITIATED (v3.9.2)");
    dotenvy::dotenv().ok();

    let api_key = env::var("GEMINI_API_KEY")
        .expect("[MAIN] Fatal: GEMINI_API_KEY not found in .env file.");

    let (tx, _) = broadcast::channel(256);
    nexus_singularity::logger::init_tracing(tx.clone());

    let _watcher = nexus_singularity::fs_watcher::NexusWatcher::ignite(tx.clone())
        .expect("[REPAIR] Failed to ignite FS_WATCHER");

    if let Err(e) = security::write_startup_token() {
        eprintln!(
            "\x1b[31m!! [SECURITY] Could not write startup token: {}. \
             Ensure NEXUS_AUTH_KEY (>= 32 bytes) is set.\x1b[0m", e
        );
    }

    let memory = Arc::new(Mutex::new(
        MemoryPalace::new(".nexus_memory.db")
            .await
            .expect("[MAIN] Fatal: cannot open MemoryPalace SQLite"),
    ));

    let agent = Arc::new(
        nexus_singularity::neural_agent::MicroAgent::init(tx.clone())
            .expect("[MAIN] Fatal: MicroAgent init failed"),
    );

    let monitor        = Arc::new(Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()));
    let vmo_state      = Arc::new(Mutex::new(VmoState::default()));
    let cluster        = Arc::new(nexus_singularity::cluster::ClusterMesh::new());
    let governance_log = Arc::new(parking_lot::Mutex::new(
        nexus_singularity::governance::protocol::GovernanceLog::init(),
    ));
    let dist = Arc::new(nexus_singularity::distributed_scheduler::DistributedScheduler::new(cluster.clone()));
    let orch = Arc::new(Orchestrator::new());

    let scheduler_tx = nexus_singularity::scheduler::MasterSupervisor::ignite(
        tx.clone(),
        reqwest::Client::new(),
        api_key.clone(),
        orch.clone(),
        Arc::new(std::sync::atomic::AtomicU32::new(50)),
        nexus_singularity::system_monitor::SystemMonitor::new(),
        memory.clone(),
        vmo_state.clone(),
        dist.clone(),
    );

    let workspace_root = env::current_dir()
        .expect("[MAIN] Fatal: cannot resolve working directory");

    let state = Arc::new(AppState {
        client:            reqwest::Client::new(),
        monitor:           monitor.clone(),
        agent:             agent.clone(),
        orchestrator:      orch,
        memory:            memory.clone(),
        api_key:           api_key.clone(),
        current_resonance: Arc::new(std::sync::atomic::AtomicU32::new(50)),
        tx:                tx.clone(),
        scheduler_tx,
        arbiter:           Arc::new(ArbiterAgent::new(agent.clone())),
        workspace_root,
        vmo_state,
        cluster:           cluster.clone(),
        dist:              dist.clone(),
        governance_log,
    });

    // ── Background subsystems ─────────────────────────────────────────────
    let hb_mesh = cluster.clone();
    tokio::spawn(async move {
        nexus_singularity::heartbeat::start(
            hb_mesh,
            env::var("NEXUS_NODE_ID").unwrap_or_else(|_| "DUET_CORE".into()),
            3000,
        ).await;
    });

    let s_state = state.clone();
    tokio::spawn(async move { start_server(s_state).await; });

    let h_state  = state.clone();
    let h_memory = memory.clone();
    tokio::spawn(async move {
        SelfHealingEngine::new(h_state, h_memory).ignite_autonomic_loop().await;
    });

    let d_state = state.clone();
    let d_memory = memory.clone();
    let d_agent = agent.clone();
    let d_api_key = api_key.clone();
    tokio::spawn(async move {
        nexus_singularity::dream_cycle::start_daydream(
            d_memory,
            d_agent,
            d_api_key,
        ).await;
    });

    let b_state = state.clone();
    let b_monitor = monitor.clone();
    tokio::spawn(async move {
        nexus_singularity::broker::KnowledgeBroker::refresh_atlas().await;
        nexus_singularity::telemetry_bridge::start_thermal_bridge(b_monitor).await;
    });

    if let Err(e) = run_cli_loop(state).await {
        eprintln!("!! [FATAL] CLI crashed: {}", e);
    }
}
EOF
ok "src/main.rs"

# =============================================================================
# 9. Local Inference Purge & Workspace Bleed Alignment
# =============================================================================
log "Purging local inference to guarantee zero fragmentation..."
rm -f src/litert_bridge.rs src/services/litert.rs src/pruner.zig
sed -i '/pub mod litert_bridge;/d' src/lib.rs
sed -i '/pub mod litert;/d' src/services/mod.rs

cat << 'EOF' > src/agents/pruner.rs
use anyhow::Result;
pub fn summarize(_input: &str) -> Result<String> {
    Ok("Pruner operation temporarily suspended pending API-only refactor.".to_string())
}
EOF
ok "Local Inference Pathways Purged"

log "Applying surgical sutures to peripheral binaries and bridges..."
sed -i 's/nexus_substrate::memory::MemoryPalace,//g' src/self_healing_engine.rs
sed -i '1i use nexus_substrate::memory::MemoryPalace;' src/self_healing_engine.rs

sed -i 's/kernel::ffi::zx_vmo_write_native/nexus_substrate::kernel::ffi::zx_vmo_write_native/g' src/telemetry_bridge.rs
sed -i 's/kernel::zx_vmo_create_native/nexus_substrate::kernel::ffi::zx_vmo_create_native/g' src/telemetry_bridge.rs
sed -i 's/crate::kernel::ffi::set_substrate_load/nexus_substrate::kernel::ffi::set_substrate_load/g' src/system_monitor.rs

sed -i 's/|s| s.to_str()/|s: \&std::ffi::OsStr| s.to_str()/g' src/bin/global_audit.rs
sed -i 's/fs::read_to_string(&file_path)/std::fs::read_to_string::<\&std::path::PathBuf>(\&file_path)/g' src/bin/global_audit.rs
ok "Workspace Bleed Cleanup Applied"
echo ""
echo -e "${CYAN}================================================${RESET}"
echo -e "${GREEN}  NEXUS CORTEX UPGRADE COMPLETE${RESET}"
echo -e "${CYAN}================================================${RESET}"
echo ""
echo -e "${YELLOW}Next steps:${RESET}"
echo "  1. Verify GEMINI_API_KEY is secured in your local .env file."
echo "  2. Execute ./universal_ignite.sh to dynamically seat libraries and recompile the DNA."
echo "  3. cargo run"
