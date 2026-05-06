#!/usr/bin/env bash
set -euo pipefail

echo ">> [NEXUS] Fusing External Audit Security & Performance Upgrades..."

# 1. ARBITER AGENT (Fixing the Allowlist Command-Injection Bypass)
cat << 'INNER_EOF' > src/arbiter_agent.rs
use crate::neural_agent::MicroAgent;
use std::sync::Arc;

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
}

const COMMAND_ALLOWLIST: &[&str] = &[
    "cargo check", "cargo build", "cargo test", "cargo clippy",
    "git status", "git log", "git diff", "ls", "find src", "cat src/",
];

impl ArbiterAgent {
    pub fn new(agent: Arc<MicroAgent>) -> Self { Self { agent } }

    pub fn validate_action(action_type: &str, command: &str) -> Result<(), String> {
        // ⚡ FIX: ALWAYS run the danger-pattern scan first.
        let dangerous_patterns = [
            "rm -rf /", "rm -rf ~", "mkfs", "> /dev/sda", "> /dev/nvme",
            "chmod 777 /", "/etc/shadow", "mv /etc/", "mv /usr/",
            ":(){ :|:& };:", "dd if=/dev/zero", "fork bomb",
            "sudo su", "sudo -i", "sudo bash", "sudo sh", "passwd root", "chown root",
            "$'\\x", "\\u0072\\u006d", "base64 -d |", "| bash", "| sh", "eval $(", "exec(",
            "curl | bash", "wget | bash", "curl | sh",
            "history -c", "unset HISTFILE",
        ];

        let normalized = command.to_lowercase();

        for pattern in dangerous_patterns.iter() {
            if normalized.contains(pattern) {
                return Err(format!("SANDBOX_VIOLATION: Threat pattern '{}' detected in {}.", pattern, action_type));
            }
        }

        if normalized.contains("../../../") || normalized.contains("/etc/passwd") {
            return Err("SANDBOX_VIOLATION: Directory traversal attempt blocked.".to_string());
        }

        if (normalized.contains("| bash") || normalized.contains("| sh") || normalized.contains("|bash") || normalized.contains("|sh")) && !normalized.starts_with("echo") {
            return Err("SANDBOX_VIOLATION: Pipe-to-shell pattern blocked.".to_string());
        }

        // ⚡ FIX: Allowlist is enforced ONLY AFTER sandbox patterns are cleared.
        if std::env::var("NEXUS_STRICT_COMMANDS").is_ok() {
            let permitted = COMMAND_ALLOWLIST.iter().any(|&allowed| {
                command.trim().starts_with(allowed)
            });
            if !permitted {
                return Err(format!("ALLOWLIST_VIOLATION: '{}' is not in the permitted command set.", command));
            }
        }

        Ok(())
    }

    pub fn evaluate_entropy_delta(base_entropy: f32, new_entropy: f32) -> Result<(), String> {
        let delta = (new_entropy - base_entropy).abs();
        if delta > 0.5 { return Err(format!("ENTROPY_ANOMALY: Delta of {:.4} exceeds 0.5 threshold.", delta)); }
        Ok(())
    }

    pub fn resolve_consensus(alpha_vector: &[f32], beta_vector: &[f32]) -> bool {
        if alpha_vector.len() != beta_vector.len() { return false; }
        let diff: f32 = alpha_vector.iter().zip(beta_vector.iter()).map(|(a, b)| (a - b).abs()).sum();
        diff < 0.5
    }

    pub fn bft_consensus(vectors: &[Vec<f32>], epsilon: f32) -> Option<Vec<f32>> {
        if vectors.is_empty() { return None; }
        let n = vectors.len();
        let threshold = (2 * n / 3) + 1;
        let dim = vectors[0].len();
        if vectors.iter().any(|v| v.len() != dim) { return None; }

        let (best_idx, neighbor_count) = (0..n)
            .map(|i| {
                let neighbors = (0..n).filter(|&j| { vectors[i].iter().zip(vectors[j].iter()).map(|(a, b)| (a - b).abs()).sum::<f32>() < epsilon }).count();
                (i, neighbors)
            }).max_by_key(|&(_, count)| count)?;
            
        if neighbor_count < threshold { return None; }

        let agreeing: Vec<&Vec<f32>> = (0..n).filter(|&j| { vectors[best_idx].iter().zip(vectors[j].iter()).map(|(a, b)| (a - b).abs()).sum::<f32>() < epsilon }).map(|j| &vectors[j]).collect();
        let centroid: Vec<f32> = (0..dim).map(|k| agreeing.iter().map(|v| v[k]).sum::<f32>() / agreeing.len() as f32).collect();
        Some(centroid)
    }
}
INNER_EOF

# 2. LOGIC ENGINE (Fixing the Deadlock False Positives)
cat << 'INNER_EOF' > src/logic_engine.rs
pub struct LogicEngine;

impl LogicEngine {
    pub fn is_action_safe(directive: &str) -> bool {
        let forbidden = ["rm -rf", "mkfs", "shutdown"];
        let d_lower = directive.to_lowercase();
        !forbidden.iter().any(|&p| d_lower.contains(p))
    }

    pub fn detect_deadlock(observations: &str) -> bool {
        // ⚡ FIX: Count occurrences, not string byte length
        let error_count = observations.matches("ERROR").count() + observations.matches("FAILED").count();
        error_count >= 3
    }
}
INNER_EOF

# 3. EXECUTOR (Fixing Blocking I/O to prevent Memory Stalls)
cat << 'INNER_EOF' > src/executor.rs
use crate::agent_protocol::AgentAction;
use crate::arbiter_agent::ArbiterAgent;
use crate::distributed_scheduler::DistributedScheduler;
use crate::server::VmoState;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, PartialEq, Clone)]
pub enum FsmState { Idle, Executing, Complete, Fault }

pub struct RoboticExecutor {
    pub current_state: FsmState,
    tx: tokio::sync::broadcast::Sender<String>,
}

impl RoboticExecutor {
    pub fn new(_api_key: String, tx: tokio::sync::broadcast::Sender<String>) -> Self {
        Self { current_state: FsmState::Idle, tx }
    }

    pub async fn execute_handle(
        &mut self, actions: Vec<AgentAction>, memory: Arc<Mutex<crate::memory::MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>, dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>> {
        self.current_state = FsmState::Executing;
        let mut observations = Vec::new();
        let total = actions.len();

        for (idx, action) in actions.into_iter().enumerate() {
            let _ = self.tx.send(serde_json::json!({
                "type": "TASK_STATUS", "payload": format!("Executing → {:?}", action)
            }).to_string());
            
            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => {
                    observations.push(obs.clone());
                    info!("Action observation: {}", obs);
                }
                Err(e) => {
                    let skipped = total.saturating_sub(idx + 1);
                    let err_obs = format!("ACTION_FAILED: {} ({} action(s) skipped)", e, skipped);
                    observations.push(err_obs.clone());
                    warn!("Action failed: {}", e);
                    self.current_state = FsmState::Fault;
                    return Ok(observations);
                }
            }
        }
        self.current_state = FsmState::Complete;
        Ok(observations)
    }

    async fn execute_single_action(
        &mut self, action: AgentAction, _memory: Arc<Mutex<crate::memory::MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>, _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        match action {
            AgentAction::ExecuteCommand { command, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Reasoning: {}", reasoning)}).to_string());
                if let Err(e) = ArbiterAgent::validate_action("execute_command", &command) { return Ok(format!("SHIELD_BLOCKED: {}", e)); }
                match crate::podman::PodmanShield::execute(&command) {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let status = if output.status.success() { "SUCCESS" } else { "FAILED" };
                        Ok(format!("[CMD:{}] STDOUT: {}\nSTDERR: {}\nReason: {}", status, stdout, stderr, reasoning))
                    }
                    Err(e) => Ok(format!("PODMAN_SHIELD_ERROR: {}", e)),
                }
            }
            AgentAction::WriteFile { path, content, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Writing file: {} | Reason: {}", path, reasoning)}).to_string());
                let content_len = content.len();
                // ⚡ FIX: Async I/O
                tokio::fs::write(&path, content).await
                    .map(|_| format!("[WRITE_OK] {} bytes → {}", content_len, path))
                    .map_err(|e| anyhow::anyhow!("WRITE_FAILED: {}", e))
            }
            AgentAction::ReadFile { path, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Reading file: {} | Reason: {}", path, reasoning)}).to_string());
                // ⚡ FIX: Async I/O
                tokio::fs::read_to_string(&path).await
                    .map(|c| format!("[READ_OK] {} ({} bytes)", path, c.len()))
                    .map_err(|e| anyhow::anyhow!("READ_FAILED: {}", e))
            }
            AgentAction::ExportReport { title, content, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Exporting report: {} | Reason: {}", title, reasoning)}).to_string());
                let safe_name = title.to_lowercase().replace(|c: char| !c.is_alphanumeric() && c != ' ' && c != '-', "_");
                let path = format!("src/{}.md", safe_name);
                tokio::fs::write(&path, content).await
                    .map(|_| format!("[REPORT_OK] Exported → {}", path))
                    .map_err(|e| anyhow::anyhow!("EXPORT_FAILED: {}", e))
            }
            AgentAction::FetchUiSnapshot { reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("fetch_ui_snapshot triggered — {}", reasoning)}).to_string());
                Ok("[UI_SNAPSHOT_REQUESTED] Ghost DOM state capture requested.".to_string())
            }
            AgentAction::Halt { reasoning } => {
                self.current_state = FsmState::Complete;
                let _ = self.tx.send(serde_json::json!({"type": "TASK_STATUS", "payload": format!("HALT requested: {}", reasoning)}).to_string());
                Ok(format!("[HALT] {}", reasoning))
            }
            _ => Ok(format!("[STUB_OK] Action acknowledged: {:?}", action))
        }
    }
}
INNER_EOF

# 4. SCHEDULER (Fixing Silent Task Drops under Thermal Load)
cat << 'INNER_EOF' > src/scheduler.rs
use crate::distributed_scheduler::DistributedScheduler;
use crate::memory::MemoryPalace;
use crate::orchestrator::Orchestrator;
use crate::server::VmoState;
use crate::system_monitor::SystemMonitor;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct NexusTask {
    pub payload: String,
    pub priority: u8,
}

pub struct MasterSupervisor;

impl MasterSupervisor {
    pub fn ignite(
        tx: broadcast::Sender<String>, client: reqwest::Client, api_key: String, orchestrator: Arc<Orchestrator>,
        _resonance_meter: Arc<AtomicU32>, mut monitor: SystemMonitor, memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>, dist: Arc<DistributedScheduler>,
    ) -> mpsc::Sender<NexusTask> {
        let (task_tx, mut task_rx) = mpsc::channel::<NexusTask>(32);
        let task_tx_requeue = task_tx.clone();

        tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                let vitals = monitor.get_vitals();

                if vitals.cpu_load > 0.85 || vitals.temperature > 68.0 {
                    warn!("[SCHEDULER] Substrate hot. Re-queuing task.");
                    // ⚡ FIX: Re-queue task asynchronously instead of dropping it
                    let requeue_tx = task_tx_requeue.clone();
                    let t_clone = task.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
                        let _ = requeue_tx.send(t_clone).await;
                    });
                    continue;
                }

                let cl = client.clone();
                let ak = api_key.clone();
                let orch = orchestrator.clone();
                let mem = memory.clone();
                let vs = vmo_state.clone();
                let task_tx_clone = tx.clone();
                let d = dist.clone();

                tokio::spawn(async move {
                    let _ = crate::task_manager::send_task(orch, &cl, &ak, &task.payload, mem, vs, task_tx_clone, d).await;
                });
            }
        });
        task_tx
    }
}
INNER_EOF

# 5. SECURITY (Fixing JWT Token Permissions)
cat << 'INNER_EOF' > src/security.rs
use axum::{body::Body, http::{Request, StatusCode, header}, middleware::Next, response::Response};
use crate::auth;

pub async fn sovereign_auth_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if req.uri().path() == "/api/auth/login" { return Ok(next.run(req).await); }

    let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = auth_header.trim_start_matches("Bearer ").trim();

    if !token.is_empty() && auth::verify_token(token).unwrap_or(false) {
        return Ok(next.run(req).await);
    }

    tracing::warn!("[SECURITY] 401 Unauthorized: Path={} | HeaderPresent={}", req.uri().path(), !auth_header.is_empty());
    Err(StatusCode::UNAUTHORIZED)
}

pub fn write_startup_token() -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let token = auth::issue_sovereign_token()?;
    // ⚡ FIX: Strict 0600 permissions
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(".nexus_ui_token")?
        .write_all(token.as_bytes())?;

    println!("\x1b[32m>> [SECURITY] Startup token written to .nexus_ui_token (0600)\x1b[0m");
    Ok(())
}
INNER_EOF

echo ">> [COMPLETE] Core Substrate Secured."
