#!/usr/bin/env bash
# ghost_containment_patch.sh
set -euo pipefail

cd /home/donball197/nexus_singularity-dev || exit 1
echo "🛠️ Applying Ghost Session Containment Patch..."

# 1. FIX: Guard inject_workspace_dna against empty directives
cat << 'EOF' > src/cli.rs
// src/cli.rs — v6.2 [GHOST_SESSION_GUARD]
use crate::task_manager;
use std::io::{self, Write};
use std::sync::Arc;

pub async fn inject_workspace_dna(directive: &str) -> String {
    // ⚡ CONTAINMENT: Instantly halt if the directive is a ghost pulse
    if directive.trim().is_empty() {
        return "No directive provided. Halt immediately.".to_string();
    }
    directive.trim().to_string()
}

pub async fn get_execution_law_v6() -> String {
    r#"You are NEXUS — a silent, precise, rule-bound autonomous coding agent.

=== ABSOLUTE OUTPUT RULES (VIOLATION = SYSTEM FAILURE) ===
- Output **ONLY** a valid JSON array of AgentAction objects.
- NO explanations, NO thoughts, NO markdown, NO "I think", NO preamble.
- Response must start with `[` and end with `]`.
- Keep "reasoning" fields very short (max 12 words).

=== ONE LOGICAL STEP PER TURN ===
- Do exactly one meaningful action per response.
- After read_file / diff_patch / write_file / execute_command, stop and wait for next turn.
- If mission is complete based on observations, output only `halt`.

=== EDITING PREFERENCE ===
- Prefer `diff_patch` for small surgical changes.
- Use `patch_function` only when asked for semantic/AST-level function replacement.
- Use `read_file` first when you need to see current code.

=== READ_FILE PROTOCOL ===
- When you see [READ_OK] with --- CONTENT --- block, use that content directly.
- Never re-read the same file if content is already provided.

Valid actions only:
- "read_file"      { "path": "...", "reasoning": "..." }
- "write_file"     { "path": "...", "content": "...", "reasoning": "..." }
- "diff_patch"     { "path": "...", "search": "...", "replace": "...", "reasoning": "..." }
- "patch_function" { "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." }
- "execute_command"{ "command": "...", "reasoning": "..." }
- "halt"           { "reasoning": "..." }

Now execute the directive. Output ONLY the JSON array."#.to_string()
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    loop {
        print!("\n\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        io::stdout().flush().ok();
        let mut input = String::new();
        if stdin.read_line(&mut input)? == 0 { break; }
        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "exit" || trimmed == "quit" {
            println!("\x1b[33m>> [CLI] Graceful shutdown requested.\x1b[0m");
            break;
        }

        let mut rx = state.tx.subscribe();
        let stream_handle = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                    if val["type"] == "NEURAL_STREAM" {
                        if let Some(token) = val["payload"].as_str() {
                            print!("\x1b[38;5;33m{}\x1b[0m", token);
                            std::io::stdout().flush().ok();
                        }
                    }
                }
            }
        });

        task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            trimmed,
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
            state.dist.clone(),
        ).await;

        stream_handle.abort();
    }
    Ok(())
}
EOF

# 2. FIX: Explicitly secure FS_Watcher to prevent autonomous task dispatch
cat << 'EOF' > src/fs_watcher.rs
use notify_debouncer_full::{new_debouncer, notify::*, DebounceEventResult, Debouncer, NoCache};
use std::time::Duration;
use tracing::{info, warn, debug};
use tokio::sync::broadcast;

pub struct NexusWatcher;

impl NexusWatcher {
    /// ignite: Starts the OS-level event loop with Warp-style debouncing.
    /// ⚡ CONTAINMENT: Strictly bound to telemetry. Autonomous agent triggers are explicitly forbidden.
    pub fn ignite(tx: broadcast::Sender<String>) -> anyhow::Result<Debouncer<RecommendedWatcher, NoCache>> {
        let tx_clone = tx.clone();
        
        let mut debouncer = new_debouncer(Duration::from_millis(200), None, move |res: DebounceEventResult| {
            match res {
                Ok(events) => {
                    for event in events {
                        let paths: Vec<String> = event.event.paths.iter()
                            .map(|p| p.to_string_lossy().to_string()).collect();
                        
                        let msg = match event.event.kind {
                            EventKind::Modify(_) => format!("FS_MODIFIED: {:?}", paths),
                            EventKind::Create(_) => format!("FS_CREATED: {:?}", paths),
                            EventKind::Remove(_) => format!("FS_DELETED: {:?}", paths),
                            _ => continue,
                        };
                        
                        let _ = tx_clone.send(serde_json::json!({
                            "type": "LOG_EVENT",
                            "payload": msg
                        }).to_string());
                        debug!("{}", msg);
                    }
                }
                Err(e) => warn!("FS_WATCHER_ERROR: {:?}", e),
            }
        })?;

        debouncer.watch(std::path::Path::new("./src"), RecursiveMode::Recursive)?;
        info!("\x1b[36m>> [AUTONOMIC_WATCHER] OS-Level Hooks Active (Agent Dispatch DISABLED)\x1b[0m");
        Ok(debouncer)
    }
}
EOF

# 3. FIX: Add 'file' and 'sed' to the shell router heuristic
sed -i 's/|| payload_lower.starts_with("head")/|| payload_lower.starts_with("head") || payload_lower.starts_with("sed") || payload_lower.starts_with("file")/' src/agents/agentic_runtime.rs

echo "🚀 Building Hardened Substrate..."
cargo build -j 1 --bin nexus

echo "✅ Ghost Containment Applied!"
