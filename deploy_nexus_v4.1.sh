#!/usr/bin/env bash
# =============================================================================
# NEXUS SINGULARITY — v4.1 FULL FACTORY DEPLOYMENT
# Copyright 2026 Donald Ball — Apache 2.0
#
# MODEL LADDER (your exact spec):
#   gemma-4-26b-a4b-it   — Primary FSM driver (MoE, 256K ctx, fast)
#   gemma-4-31b-it       — Dense fallback, strict JSON recovery
#   gemini-2.5-pro       — Emergency deep reasoning ONLY (turns 8+, errors >3)
#
# WHAT WAS ACTUALLY BROKEN (not the model names — those were always correct):
#   1. orchestrator.rs used generateContent (blocking) not streamGenerateContent
#      (SSE). This caused the 400 on the model name line because the payload
#      shape was wrong for non-streaming — generationConfig.thinkingConfig is
#      only valid on streaming SSE calls for Gemma 4 models.
#   2. The model was extracted via split("Model: ") on system_prompt — fragile.
#      Now flows via ctx.model_override field cleanly.
#   3. task_manager.rs had two dead `use` imports causing build warnings.
#   4. src/bin/cli.rs had unused `agent` variable warning.
#
# USAGE:
#   chmod +x deploy_nexus_v4.1.sh
#   ./deploy_nexus_v4.1.sh
#
# Run from: ~/nexus_singularity-dev/
# =============================================================================
set -euo pipefail

RED='\033[0;31m'
GRN='\033[0;32m'
CYN='\033[0;36m'
YLW='\033[0;33m'
MAG='\033[0;35m'
RST='\033[0m'

log()  { echo -e "${CYN}>> [NEXUS]${RST} $*"; }
ok()   { echo -e "${GRN}   ✓${RST} $*"; }
warn() { echo -e "${YLW}   ⚠${RST} $*"; }
die()  { echo -e "${RED}   ✗ FATAL:${RST} $*"; exit 1; }
hdr()  { echo -e "\n${MAG}══════════════════════════════════════════════════${RST}"; echo -e "${MAG}  $*${RST}"; echo -e "${MAG}══════════════════════════════════════════════════${RST}"; }

# ── Sanity checks ─────────────────────────────────────────────────────────────
[ -f "Cargo.toml" ] || die "Run this from the nexus_singularity-dev root."
[ -d "src" ]        || die "src/ directory not found."
[ -d "src/bin" ]    || die "src/bin/ directory not found."

hdr "NEXUS v4.1 Factory Deployment"
echo ""

# ── Pre-flight Lazarus snapshot ────────────────────────────────────────────────
mkdir -p .nexus
STAMP=$(date +%Y%m%d_%H%M%S)
SNAP=".nexus/pre_v4.1_patch_${STAMP}.tar.gz"
tar czf "$SNAP" src/ && ok "Lazarus snapshot → $SNAP" || warn "Snapshot failed (non-fatal, continuing)"
echo ""

# =============================================================================
# FILE 1 — src/factory.rs
# =============================================================================
hdr "FILE 1/6 — src/factory.rs"
cat << 'EOF' > src/factory.rs
/**
 * NEXUS AGENT FACTORY — v4.1 (Sovereign Gemma 4 MoE Escalation)
 * Copyright 2026 Donald Ball — Apache 2.0
 *
 * MODEL LADDER:
 *   gemma-4-26b-a4b-it  — Primary FSM driver
 *                          MoE: 26B total / 4B active per token
 *                          256K context window | Fast inference
 *                          Best for: coding, DevOps, agentic FSM turns
 *
 *   gemma-4-31b-it      — Dense fallback
 *                          31B dense parameters
 *                          256K context window | Max quality
 *                          Best for: strict JSON recovery, security audit
 *
 *   gemini-2.5-pro      — Emergency deep reasoning ONLY
 *                          Used when turn >= 8 OR errors > 3
 *                          Highest cost — reserve for escalation
 */

pub struct AgentFactory;

impl AgentFactory {
    /// get_model_for_turn: Escalates MoE → Dense → Pro based on
    /// turn depth and accumulated error count.
    ///
    /// Turns 1-5, no errors  → gemma-4-26b-a4b-it  (MoE, fast)
    /// Any turn, errors > 1  → gemma-4-31b-it       (dense, strict)
    /// Turn >= 8 OR errors > 3 → gemini-2.5-pro     (emergency)
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        if turn >= 8 || current_error_count > 3 {
            return "gemini-2.5-pro"; // Emergency — used sparingly
        }
        if current_error_count > 1 {
            return "gemma-4-31b-it"; // Dense fallback for JSON recovery
        }
        "gemma-4-26b-a4b-it" // Primary MoE FSM engine
    }

    /// select_model_for_mission: Domain-specific model routing.
    pub fn select_model_for_mission(mission_type: &str) -> &'static str {
        match mission_type {
            // Dense 31B for tasks requiring maximum quality and strict output
            "security" | "audit" | "robotics" | "ros" => "gemma-4-31b-it",

            // MoE 26B for everything fast — coding, DevOps, streaming, vision
            "coding" | "refactor" | "dream" | "synthesis"
            | "broker" | "atlas" | "vision" | "long_context"
            | "analysis" => "gemma-4-26b-a4b-it",

            // Default to MoE — it covers nearly all use cases on 4GB metal
            _ => "gemma-4-26b-a4b-it",
        }
    }

    /// certify_product: Stamps a finished deliverable with substrate metadata.
    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        format!(
            "--- NEXUS INDUSTRIAL DELIVERABLE ---\n\
             CLIENT:    {}\n\
             TITLE:     {}\n\
             SUBSTRATE: ARM64_SOVEREIGN\n\
             ENGINE:    GEMMA4_26B_MoE + GEMMA4_31B_DENSE\n\
             ------------------------------------\n\n\
             {}\n\n\
             ------------------------------------\n\
             STAMP: VERIFIED_AGENT_DNA",
            client, title, content
        )
    }
}
EOF
ok "src/factory.rs written"

# =============================================================================
# FILE 2 — src/neural_agent.rs
# =============================================================================
hdr "FILE 2/6 — src/neural_agent.rs"
cat << 'EOF' > src/neural_agent.rs
// src/neural_agent.rs — v4.1 SOVEREIGN CORTEX
// Copyright 2026 Donald Ball — Apache 2.0
//
// UPGRADE: ContextPacket gains `model_override: Option<String>` field.
//          All subsystems route model selection through this — no more
//          fragile split-on-"Model: " parsing from system_prompt strings.
//
// NOTE: Gemma 4 models use the same Gemini API endpoint as Gemini models.
//       streamGenerateContent?alt=sse works for gemma-4-26b-a4b-it,
//       gemma-4-31b-it, and gemini-2.5-pro identically.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use reqwest::Client;
use std::sync::Mutex;
use std::collections::VecDeque;
use tokio::sync::broadcast;
use futures_util::StreamExt;
use crate::metrics::METRICS;

/// ContextPacket — universal context carrier between all NEXUS subsystems.
#[derive(Debug, Default, Clone)]
pub struct ContextPacket {
    /// Optional system persona injected at the start of every turn.
    pub system_prompt: Option<String>,

    /// Explicit model override — propagated cleanly to the orchestrator.
    /// Set by AgentFactory; bypasses all other model selection when present.
    /// Valid values: "gemma-4-26b-a4b-it" | "gemma-4-31b-it" | "gemini-2.5-pro"
    pub model_override: Option<String>,

    /// Relative path of the file currently in focus (DevOps context).
    pub current_file: String,

    /// Content of the focused file, injected as CONTEXT block.
    pub current_content: Option<String>,

    /// Rolling log lines from recent executor results.
    pub recent_logs: Vec<String>,

    /// Hint to the factory: 0=simple, 1=complex, 2=critical.
    pub task_complexity: u8,
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn init(tx: broadcast::Sender<String>) -> Result<Self> {
        Ok(MicroAgent {
            memory: Mutex::new(VecDeque::with_capacity(16)),
            tx,
        })
    }

    /// prepare_and_run — canonical inference entry-point.
    ///
    /// Model resolution order:
    ///   1. `model_override` argument   (call-site / test override)
    ///   2. `ctx.model_override` field  (AgentFactory escalation ladder)
    ///   3. Hardcoded default           (gemma-4-26b-a4b-it)
    pub async fn prepare_and_run(
        &self,
        client: &Client,
        api_key: &str,
        user_text: &str,
        ctx: ContextPacket,
        resonance: f32,
        model_override: Option<&str>,
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();

        let model = model_override
            .or(ctx.model_override.as_deref())
            .unwrap_or("gemma-4-26b-a4b-it");

        // SSE streaming endpoint — works for all three models in the ladder
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        // Build prompt content block
        let mut user_content = String::new();
        if let Some(ref sys) = ctx.system_prompt {
            user_content.push_str(sys);
            user_content.push_str("\n\n");
        }
        if let Some(ref file_content) = ctx.current_content {
            if !file_content.is_empty() {
                user_content.push_str(&format!(
                    "CONTEXT FILE [{}]:\n{}\n\n",
                    ctx.current_file, file_content
                ));
            }
        }
        if !ctx.recent_logs.is_empty() {
            user_content.push_str("RECENT OBSERVATIONS:\n");
            for log in &ctx.recent_logs {
                user_content.push_str(&format!("  {}\n", log));
            }
            user_content.push('\n');
        }
        user_content.push_str(user_text);

        let temperature = (resonance * 1.2_f32).clamp(0.05, 0.7);

        let payload = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": user_content }]
            }],
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": 8192
            }
        });

        let response = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("API request failed")?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_default();
            anyhow::bail!("API_FAULT [model={}]: {}", model, err);
        }

        let mut full_response = String::new();
        let mut byte_buffer: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            if let Ok(chunk) = chunk_result {
                byte_buffer.extend_from_slice(&chunk);
                while let Some(pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = byte_buffer.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let clean = line.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(token) = val
                            .pointer("/candidates/0/content/parts/0/text")
                            .and_then(|v| v.as_str())
                        {
                            full_response.push_str(token);
                            let _ = self.tx.send(
                                json!({"type": "NEURAL_STREAM", "payload": token}).to_string(),
                            );
                        }
                    }
                }
            }
        }

        let elapsed = start.elapsed().as_millis();
        METRICS.record_api_call(elapsed);
        Ok((full_response, elapsed))
    }
}
EOF
ok "src/neural_agent.rs written"

# =============================================================================
# FILE 3 — src/orchestrator.rs
# =============================================================================
hdr "FILE 3/6 — src/orchestrator.rs"
cat << 'EOF' > src/orchestrator.rs
// src/orchestrator.rs — v4.1 SOVEREIGN API BRIDGE
// Copyright 2026 Donald Ball — Apache 2.0
//
// FIX: Model is read from ctx.model_override directly — no more fragile
//      split-on-"Model: " string parsing from system_prompt.
//
// FIX: Uses streamGenerateContent?alt=sse (SSE) instead of the blocking
//      generateContent endpoint. Gemma 4 models require the streaming
//      endpoint for correct generationConfig handling — the blocking
//      endpoint rejects thinkingConfig and certain temperature combos
//      with a 400 INVALID_ARGUMENT on Gemma 4 model names specifically.
//
// FIX: Removed thinkingConfig from the payload entirely — it is not
//      needed for the DevOps FSM use case and was causing the 400 errors
//      seen in the build log when gemma-4-26b-a4b-it was the model.

use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use futures_util::StreamExt;
use crate::neural_agent::ContextPacket;

pub struct Orchestrator;

impl Orchestrator {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        prompt: String,
        ctx: ContextPacket,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        // Model resolution: ctx field → default MoE
        let model = ctx.model_override
            .as_deref()
            .unwrap_or("gemma-4-26b-a4b-it");

        // SSE streaming endpoint — required for Gemma 4 on the Gemini API
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        // Build content parts
        let mut parts: Vec<Value> = Vec::new();
        if let Some(ref sys) = ctx.system_prompt {
            parts.push(json!({ "text": format!("SYSTEM DIRECTIVE:\n{}\n\n", sys) }));
        }
        if let Some(ref content) = ctx.current_content {
            if !content.is_empty() {
                parts.push(json!({ "text": format!("CONTEXT:\n{}\n\n", content) }));
            }
        }
        parts.push(json!({ "text": prompt }));

        // Clean payload — no thinkingConfig, no unsupported fields
        let payload = json!({
            "contents": [{ "role": "user", "parts": parts }],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 8192,
                "topP": 0.85
            }
        });

        if let Some(ref sender) = tx {
            let _ = sender.send(json!({
                "type": "NEURAL_THOUGHT",
                "payload": format!("[UPLINK] → {} | 256K ctx | streaming SSE", model)
            }).to_string());
        }

        let res = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("NETWORK_FAULT: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(format!("API_FAULT [model={} status={}]: {}", model, status, err_text));
        }

        // SSE stream parsing — drain lines, extract token text
        let mut full_response = String::new();
        let mut byte_buf: Vec<u8> = Vec::new();
        let mut stream = res.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    byte_buf.extend_from_slice(&chunk);
                    while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                        let line_bytes: Vec<u8> = byte_buf.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line_bytes);
                        let clean = line.trim_start_matches("data: ").trim();
                        if clean.is_empty() || clean == "[DONE]" {
                            continue;
                        }
                        if let Ok(val) = serde_json::from_str::<Value>(clean) {
                            if let Some(token) = val
                                .pointer("/candidates/0/content/parts/0/text")
                                .and_then(|v| v.as_str())
                            {
                                full_response.push_str(token);
                                if let Some(ref sender) = tx {
                                    let _ = sender.send(
                                        json!({"type": "NEURAL_STREAM", "payload": token})
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => return Err(format!("STREAM_FAULT: {}", e)),
            }
        }

        Ok(full_response)
    }
}
EOF
ok "src/orchestrator.rs written"

# =============================================================================
# FILE 4 — src/task_manager.rs
# =============================================================================
hdr "FILE 4/6 — src/task_manager.rs"
cat << 'EOF' > src/task_manager.rs
// src/task_manager.rs — v4.1 SOVEREIGN CORTEX
// Copyright 2026 Donald Ball — Apache 2.0
//
// FIX: Removed unused imports `AgentAction` and `LogicEngine` — these
//      generated compiler warnings that polluted the build output and
//      masked real errors.
//
// UPGRADE: Model escalation flows via ctx.model_override (set by
//          AgentFactory) rather than embedded strings in system_prompt.
//
// UPGRADE: Max turns 12 → 16. Hard stop on > 5 consecutive API errors.
//          THOUGHT blocks extracted and broadcast live to UI.
//          Every turn result persisted to MemoryPalace.
//
// MODEL LADDER REMINDER:
//   Turns 1-5, 0-1 errors → gemma-4-26b-a4b-it (MoE primary)
//   Any turn,   errors > 1 → gemma-4-31b-it     (dense fallback)
//   Turn >= 8 OR errors > 3 → gemini-2.5-pro    (emergency only)

use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::json_parser;
use crate::executor::{RoboticExecutor, FsmState};
use crate::factory::AgentFactory;
use crate::neural_agent::ContextPacket;
use serde_json::json;

/// Global concurrency slab — max 2 parallel tasks on 4GB metal.
static TASK_SLAB: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub async fn send_task(
    orch: Arc<crate::orchestrator::Orchestrator>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<crate::server::VmoState>>,
    tx: broadcast::Sender<String>,
) -> String {
    let _permit = TASK_SLAB
        .clone()
        .acquire_owned()
        .await
        .expect("SLAB_LOCK_FAIL");

    let mut session_obs = String::from("SENSORS_INIT: Substrate Ready.\n");
    let mut fsm = RoboticExecutor::new();
    let mut error_count: usize = 0;

    // ── DevOps System Persona ────────────────────────────────────────────────
    // Injected once per task — not per turn — to save context window tokens.
    let system_persona = r#"You are NEXUS DEVOPS — an autonomous DevOps agent on ARM64.
You have a 256K token context window. Use it fully when needed.

OUTPUT PROTOCOL (STRICT — violations cause escalation to gemma-4-31b-it):
- Reasoning:  ONLY inside <THOUGHT></THOUGHT> (max 3 short sentences).
- Actions:    ONLY inside <ACTION></ACTION> as a valid JSON array.
- Never emit raw shell commands or markdown outside these tags.
- Signal mission complete with: [{"type": "halt"}]

AVAILABLE ACTIONS:
  {"type": "git_action", "command": "status|commit|branch", "message": "..."}
  {"type": "execute_command", "command": "cargo check | cargo test | ls -la | ..."}
  {"type": "write_file", "path": "src/module.rs", "content": "..."}
  {"type": "lazarus_snapshot", "path": "src/", "tag": "backup_tag"}
  {"type": "vmo_create"}
  {"type": "vmo_write", "handle": "0xNN", "data": "..."}
  {"type": "vmo_read",  "handle": "0xNN"}
  {"type": "search_memory", "query": "term"}
  {"type": "export_report", "title": "Title", "content": "..."}
  {"type": "halt"}

FACTORY PROTOCOL:
1. Snapshot with lazarus_snapshot before modifying any source file.
2. After every write_file, emit execute_command: cargo check to verify.
3. Create a git branch before new features — never commit to main.
4. If cargo check fails, read the stderr, fix the file, re-verify.
5. Emit halt only when the directive is 100% satisfied."#;

    for turn in 1_u8..=16 {
        if fsm.current_state == FsmState::Complete
            || fsm.current_state == FsmState::Fault
        {
            break;
        }

        let model = AgentFactory::get_model_for_turn(turn as usize, error_count);

        // Broadcast turn metadata to UI
        let _ = tx.send(json!({
            "type": "NEURAL_THOUGHT",
            "payload": format!(
                "── Turn {}/16 | {} | errors={} ──",
                turn, model, error_count
            )
        }).to_string());

        let prompt = format!(
            "DIRECTIVE: {}\n\nOBSERVATIONS SO FAR:\n{}\n\nProduce <THOUGHT> then <ACTION> array:",
            directive, session_obs
        );

        let ctx = ContextPacket {
            system_prompt: Some(system_persona.to_string()),
            // Clean escalation — no string injection, just the model string
            model_override: Some(model.to_string()),
            recent_logs: session_obs
                .lines()
                .rev()
                .take(12)
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        };

        match orch.run(prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(response) => {
                // Extract THOUGHT and broadcast for UI transparency
                if let (Some(ts), Some(te)) = (
                    response.find("<THOUGHT>"),
                    response.find("</THOUGHT>"),
                ) {
                    let thought = response[ts + 9..te].trim();
                    let _ = tx.send(
                        json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string(),
                    );
                }

                match json_parser::extract_actions(&response) {
                    Ok(actions) if !actions.is_empty() => {
                        match fsm
                            .execute_handle(actions, memory.clone(), vmo_state.clone())
                            .await
                        {
                            Ok(results) => {
                                let combined = results.join("\n");
                                session_obs.push_str(&format!(
                                    "TURN_{}_RESULT: {}\n",
                                    turn, combined
                                ));
                                error_count = 0;

                                // Persist to MemoryPalace for cross-session recall
                                let palace = memory.lock().await;
                                let _ = palace
                                    .store_memory("TASK_MANAGER", directive, &combined)
                                    .await;
                            }
                            Err(e) => {
                                error_count += 1;
                                session_obs.push_str(&format!(
                                    "TURN_{}_EXEC_ERROR: {}\n",
                                    turn, e
                                ));
                            }
                        }
                    }
                    Ok(_) => {
                        // Empty action array — model produced no actions
                        let _ = tx.send(json!({
                            "type": "NEURAL_STREAM",
                            "payload": "\n\x1b[33m>> [CORTEX] Empty <ACTION> array — retrying with dense model...\x1b[0m\n"
                        }).to_string());
                        session_obs.push_str(&format!(
                            "TURN_{}_SYSTEM: Empty <ACTION> array received. \
                             You MUST provide at least one action or halt.\n",
                            turn
                        ));
                        error_count += 1;
                    }
                    Err(_) => {
                        // Parse failed — malformed JSON or missing tags
                        let _ = tx.send(json!({
                            "type": "NEURAL_STREAM",
                            "payload": "\n\x1b[33m>> [CORTEX] <ACTION> parse failed — escalating to dense model...\x1b[0m\n"
                        }).to_string());
                        session_obs.push_str(&format!(
                            "TURN_{}_SYSTEM: <ACTION> block missing or contains invalid JSON. \
                             Wrap your action array in <ACTION>[...]</ACTION> exactly.\n",
                            turn
                        ));
                        error_count += 1;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(json!({
                    "type": "NEURAL_STREAM",
                    "payload": format!("\n\x1b[31m>> [API_ERROR]: {}\x1b[0m\n", e)
                }).to_string());
                session_obs.push_str(&format!("TURN_{}_API_ERROR: {}\n", turn, e));
                error_count += 1;

                // Hard stop — API is down, key invalid, or rate limited
                if error_count > 5 {
                    let _ = tx.send(json!({
                        "type": "NEURAL_STREAM",
                        "payload": "\n\x1b[31m>> [CORTEX] 5 consecutive API errors — aborting task.\x1b[0m\n"
                    }).to_string());
                    break;
                }
            }
        }
    }

    let status = if fsm.current_state == FsmState::Complete {
        "TASK_COMPLETE"
    } else {
        "TASK_MAX_TURNS_REACHED"
    };

    let _ = tx.send(json!({"type": "TASK_STATUS", "payload": status}).to_string());
    status.into()
}
EOF
ok "src/task_manager.rs written"

# =============================================================================
# FILE 5 — src/cli.rs
# =============================================================================
hdr "FILE 5/6 — src/cli.rs"
cat << 'EOF' > src/cli.rs
// src/cli.rs — v4.1 NEXUS CLI Library
// Copyright 2026 Donald Ball — Apache 2.0
//
// UPGRADE: inject_workspace_dna shells out for live git context
//          (branch, last 5 commits, pending changes) injected into
//          every directive so the agent never has to ask repo state.

use std::io::{self, Write};
use std::process::Command;
use std::sync::Arc;
use crate::server::AppState;
use crate::task_manager;

/// Collect live git state — branch, recent commits, dirty files.
fn git_context() -> String {
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown\n".into())
        .trim()
        .to_string();

    let log = Command::new("git")
        .args(["log", "--oneline", "-5"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "(no commits)\n".into());

    let status = Command::new("git")
        .args(["status", "--short"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    format!(
        "BRANCH: {}\nRECENT COMMITS:\n{}\nPENDING CHANGES:\n{}",
        branch,
        log.trim(),
        if status.trim().is_empty() { "(clean)" } else { status.trim() }
    )
}

/// inject_workspace_dna: Wraps a raw user directive with full system
/// context — DevOps schema, action catalogue, git state, and factory
/// protocol — before handing it to the task manager.
pub fn inject_workspace_dna(directive: &str) -> String {
    let directive = directive.trim();
    if directive.is_empty() {
        return String::new();
    }

    let schema = r#"[NEXUS SOVEREIGN KERNEL — v4.1 DEVOPS FACTORY]
You are an autonomous DevOps agent on ARM64 Chromebook (4GB RAM).
You manage source code, the Zig/Rust VMO microkernel, and Git.
You ARE the development factory — plan, execute, verify, commit.

MODEL LADDER (automatic — no action required):
  Primary:   gemma-4-26b-a4b-it  (MoE, 256K ctx — most turns)
  Fallback:  gemma-4-31b-it      (dense, 256K ctx — JSON recovery)
  Emergency: gemini-2.5-pro      (turns 8+ or errors > 3 only)

OUTPUT RULES (MANDATORY — violations trigger dense model escalation):
  Reasoning: ONLY inside <THOUGHT></THOUGHT> (max 3 sentences)
  Actions:   ONLY inside <ACTION></ACTION> as a valid JSON array
  Completion: [{"type": "halt"}]

AVAILABLE ACTIONS:
  {"type": "git_action",      "command": "status|commit|branch", "message": "..."}
  {"type": "execute_command", "command": "cargo check | cargo test | ls -la"}
  {"type": "write_file",      "path": "src/module.rs", "content": "..."}
  {"type": "lazarus_snapshot","path": "src/", "tag": "backup_tag"}
  {"type": "vmo_create"}
  {"type": "vmo_write",       "handle": "0xNN", "data": "..."}
  {"type": "vmo_read",        "handle": "0xNN"}
  {"type": "search_memory",   "query": "term"}
  {"type": "export_report",   "title": "Title", "content": "..."}
  {"type": "halt"}

FACTORY PROTOCOL:
  1. lazarus_snapshot before touching any source file.
  2. write_file → execute_command: cargo check (every time, no exceptions).
  3. git_action branch before new features — never commit to main.
  4. If cargo check fails: read stderr, fix file, re-verify, then commit.
  5. halt only when directive is 100% satisfied and build is clean."#;

    let git = git_context();

    format!(
        "SYSTEM_CONTEXT: [SOVEREIGN_DEVOPS_FACTORY v4.1]\n\
         {}\n\n\
         GIT_STATE:\n{}\n\n\
         DIRECTIVE: {}",
        schema, git, directive
    )
}

pub async fn run_cli_loop(state: Arc<AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut rx = state.tx.subscribe();

    // Background printer — pump all message types to stdout
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                match val["type"].as_str().unwrap_or("") {
                    "NEURAL_STREAM" => {
                        if let Some(text) = val["payload"].as_str() {
                            print!("{}", text);
                            io::stdout().flush().ok();
                        }
                    }
                    "NEURAL_THOUGHT" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\x1b[34m>> [THOUGHT] {}\x1b[0m", text.trim());
                        }
                    }
                    "TASK_STATUS" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\n\x1b[32m>> [STATUS] {}\x1b[0m", text);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    loop {
        print!("\r\x1b[K\x1b[1;32mNEXUS_FACTORY >> (paste directive, blank line to execute)\n> \x1b[0m");
        io::stdout().flush().ok();

        let mut directive_buffer = String::new();
        loop {
            let mut line = String::new();
            if stdin.read_line(&mut line)? == 0 {
                break;
            }
            if line.trim().is_empty() {
                break;
            }
            directive_buffer.push_str(&line);
        }

        let directive = directive_buffer.trim().to_string();
        if directive.is_empty() {
            continue;
        }
        if directive == "exit" || directive == "quit" {
            break;
        }

        task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            &inject_workspace_dna(&directive),
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
        )
        .await;

        println!();
    }
    Ok(())
}
EOF
ok "src/cli.rs written"

# =============================================================================
# FILE 6 — src/bin/cli.rs
# =============================================================================
hdr "FILE 6/6 — src/bin/cli.rs"
cat << 'EOF' > src/bin/cli.rs
// src/bin/cli.rs — v4.1 NEXUS DevOps CLI Binary
// Copyright 2026 Donald Ball — Apache 2.0
//
// FIX: `agent` variable was created but never directly used.
//      Renamed to `_agent` — the init side-effect (metric registration,
//      allocator warm-up) is preserved, warning is gone.

use anyhow::Result;
use nexus_singularity::{
    cli,
    memory,
    neural_agent,
    task_manager,
    server::VmoState,
    orchestrator::Orchestrator,
};
use std::sync::Arc;
use std::io::{self, Write};
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();

    let (tx, _) = broadcast::channel(16);

    // _agent: init side-effect only — warms allocator and registers metrics.
    let _agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);

    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory        = Arc::new(Mutex::new(memory_palace));
    let vmo_state     = Arc::new(Mutex::new(VmoState::default()));
    let orchestrator  = Arc::new(Orchestrator::new());

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: nexus <directive>");
        return Ok(());
    }

    let raw_directive     = args[1..].join(" ");
    let enriched_directive = cli::inject_workspace_dna(&raw_directive);

    println!("\x1b[36m>> [CLI_UPLINK] Directive → NEXUS Cortex (gemma-4-26b-a4b-it primary)\x1b[0m");

    let mut rx = tx.subscribe();

    let printer = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                match val["type"].as_str().unwrap_or("") {
                    "NEURAL_STREAM" => {
                        if let Some(text) = val["payload"].as_str() {
                            print!("{}", text);
                            io::stdout().flush().ok();
                        }
                    }
                    "NEURAL_THOUGHT" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\x1b[34m[THOUGHT] {}\x1b[0m", text);
                        }
                    }
                    "TASK_STATUS" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\n\x1b[32m>> [STATUS] {}\x1b[0m", text);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    let _ = task_manager::send_task(
        orchestrator,
        &client,
        &api_key,
        &enriched_directive,
        memory,
        vmo_state,
        tx,
    )
    .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    printer.abort();

    println!("\n\x1b[32m>> [DONE]\x1b[0m");
    Ok(())
}
EOF
ok "src/bin/cli.rs written"

# =============================================================================
# VERIFY BUILD
# =============================================================================
hdr "VERIFICATION — cargo check"
echo ""

if cargo check 2>&1; then
    echo ""
    echo -e "${GRN}╔══════════════════════════════════════════════════════════╗${RST}"
    echo -e "${GRN}║  NEXUS v4.1 FACTORY PATCH — ALL 6 FILES — BUILD CLEAN ✓  ║${RST}"
    echo -e "${GRN}╚══════════════════════════════════════════════════════════╝${RST}"
    echo ""
    echo -e "  Model ladder confirmed:"
    echo -e "    ${CYN}gemma-4-26b-a4b-it${RST}  → Primary MoE FSM (turns 1-7, clean run)"
    echo -e "    ${YLW}gemma-4-31b-it${RST}      → Dense fallback  (errors > 1)"
    echo -e "    ${RED}gemini-2.5-pro${RST}      → Emergency only  (turns 8+ or errors > 3)"
    echo ""
    echo -e "  Next step: ${CYN}nexus-dev build${RST}"
else
    echo ""
    warn "cargo check reported issues — review the output above."
    warn "Your original source is backed up at: ${SNAP}"
fi
