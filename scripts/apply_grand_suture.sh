#!/usr/bin/env bash
set -euo pipefail

echo ">> [NEXUS] Forging Structured Shell, HITL, and Gemma 4 MoE Routing..."

# 1. THE STRUCTURED SHELL (src/shell.rs)
cat << 'INNER_EOF' > src/shell.rs
use crate::agent_protocol::AgentAction;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

pub struct NexusShell;

impl NexusShell {
    pub async fn eval(action: &AgentAction) -> Value {
        match action {
            AgentAction::ReadFile { path, .. } => Self::builtin_read_file(path).await,
            AgentAction::WriteFile { path, content, .. } => Self::builtin_write_file(path, content).await,
            AgentAction::ExecuteCommand { command, .. } => Self::builtin_exec(command).await,
            _ => json!({ "status": "unsupported", "action": format!("{:?}", action) }),
        }
    }

    async fn builtin_read_file(path: &str) -> Value {
        let p = Path::new(path);
        match fs::read_to_string(p).await {
            Ok(content) => json!({
                "status": "success", "operation": "read_file", "path": path,
                "content": content.chars().take(2048).collect::<String>()
            }),
            Err(e) => json!({ "status": "error", "path": path, "reason": e.to_string() })
        }
    }

    async fn builtin_write_file(path: &str, content: &str) -> Value {
        let p = Path::new(path);
        if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent).await; }
        match fs::write(p, content).await {
            Ok(_) => json!({ "status": "success", "operation": "write_file", "path": path, "bytes_written": content.len() }),
            Err(e) => json!({ "status": "error", "path": path, "reason": e.to_string() })
        }
    }

    async fn builtin_exec(command: &str) -> Value {
        match tokio::process::Command::new("sh").arg("-c").arg(command).output().await {
            Ok(out) => json!({
                "status": if out.status.success() { "success" } else { "failed" },
                "command": command,
                "stdout": String::from_utf8_lossy(&out.stdout).trim().chars().take(4096).collect::<String>(),
                "stderr": String::from_utf8_lossy(&out.stderr).trim()
            }),
            Err(e) => json!({ "status": "error", "command": command, "reason": e.to_string() })
        }
    }
}
INNER_EOF

# 2. THE HITL INTERCEPTOR (src/hitl.rs)
cat << 'INNER_EOF' > src/hitl.rs
use crate::agent_protocol::AgentAction;
use std::io::{self, Write};

pub fn check_interception(action: &AgentAction) -> Result<Option<String>, String> {
    let mode = std::env::var("NEXUS_MODE").unwrap_or_else(|_| "ghost".to_string());
    if mode != "interactive" { return Ok(None); }

    match action {
        AgentAction::ExecuteCommand { command, reasoning } => {
            println!("\n\x1b[1;33m[⚠️ HITL INTERCEPT] Action: ExecuteCommand\x1b[0m");
            println!("\x1b[1;36mReasoning:\x1b[0m {}", reasoning);
            println!("\x1b[1;36mCommand:\x1b[0m {}", command);
        }
        AgentAction::WriteFile { path, content, reasoning } => {
            println!("\n\x1b[1;33m[⚠️ HITL INTERCEPT] Action: WriteFile\x1b[0m");
            println!("\x1b[1;36mReasoning:\x1b[0m {}", reasoning);
            println!("\x1b[1;36mPath:\x1b[0m {}", path);
            println!("\x1b[1;36mContent Preview:\x1b[0m {}...", content.chars().take(100).collect::<String>().replace('\n', " "));
        }
        _ => return Ok(None),
    }

    loop {
        print!("\x1b[1;35mAllow execution? [y/N/f (feedback)]: \x1b[0m");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(None),
            "n" | "no" | "" => return Err("Execution denied by Operator.".to_string()),
            "f" | "feedback" => {
                print!("\x1b[1;35mProvide correction: \x1b[0m");
                io::stdout().flush().unwrap();
                let mut f = String::new();
                io::stdin().read_line(&mut f).unwrap();
                return Err(format!("Execution denied. Feedback: {}", f.trim()));
            }
            _ => println!("Invalid input."),
        }
    }
}
INNER_EOF

# 3. THE WIRED TASK MANAGER (src/task_manager.rs)
cat << 'INNER_EOF' > src/task_manager.rs
use crate::{
    agent_protocol::AgentAction, cli, json_parser,
    orchestrator::Orchestrator, memory::MemoryPalace, server::VmoState,
    distributed_scheduler::DistributedScheduler
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};

pub async fn send_task(
    orchestrator: Arc<Orchestrator>, client: &Client, api_key: &str, raw_directive: &str,
    _memory: Arc<Mutex<MemoryPalace>>, _vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>, _dist: Arc<DistributedScheduler>,
) -> String {
    let mut session_obs = vec![];
    let mut turn = 0;
    const MAX_TURNS: usize = 8;
    let directive = cli::inject_workspace_dna(raw_directive).await;

    while turn < MAX_TURNS {
        turn += 1;
        info!("=== Turn {} started ===", turn);

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some("You are NEXUS, a precise autonomous coding agent. Follow the execution law strictly.".to_string()),
            model_override: Some("gemma-4-26b-a4b-it".to_string()),
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
                for (i, action) in actions.iter().enumerate() {
                    info!("Executing action {}/{}: {:?}", i + 1, actions.len(), action);

                    match crate::hitl::check_interception(action) {
                        Ok(None) => {
                            let obs_json = crate::shell::NexusShell::eval(action).await;
                            session_obs.push(obs_json.to_string());

                            if let AgentAction::WriteFile { .. } = action {
                                let check = AgentAction::ExecuteCommand {
                                    command: "cargo check".to_string(),
                                    reasoning: "Mandatory verification".to_string(),
                                };
                                session_obs.push(crate::shell::NexusShell::eval(&check).await.to_string());
                            }
                        }
                        Err(feedback) => {
                            session_obs.push(serde_json::json!({"status": "blocked_by_user", "reason": feedback}).to_string());
                        }
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

    if session_obs.iter().any(|o| o.contains("PARSER_FRACTURE") || o.contains("UPLINK_ERROR")) { "TASK_FAULTED".to_string() } else { "TASK_COMPLETE".to_string() }
}
INNER_EOF

# 4. ORCHESTRATOR MODEL OVERRIDE (src/orchestrator.rs)
sed -i 's/"gemma-3-27b-it"/"gemma-4-26b-a4b-it"/g' src/orchestrator.rs

# 5. REGISTER MODULES IN LIB.RS
if ! grep -q "pub mod shell;" src/lib.rs; then echo "pub mod shell;" >> src/lib.rs; fi
if ! grep -q "pub mod hitl;" src/lib.rs; then echo "pub mod hitl;" >> src/lib.rs; fi

echo ">> [COMPLETE] Grand Suture Applied."
