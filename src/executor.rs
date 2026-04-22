// src/executor.rs — v4.8 Precise DevOps Executor
// FIX: _ws_payload prefix silences unused-variable warning (was `ws_payload`).
// FIX: git_action "checkout" now correctly uses `git checkout <branch>`,
//      while "branch" creates/switches with `git checkout -B <branch>`.
use crate::agent_protocol::AgentAction;
use crate::memory::MemoryPalace;
use crate::server::VmoState;
use crate::lazarus_vault::LazarusVault;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::process::Command;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FsmState {
    Idle,
    Executing,
    Complete,
    Fault,
}

pub struct RoboticExecutor {
    pub current_state: FsmState,
}

impl RoboticExecutor {
    pub fn new() -> Self {
        Self {
            current_state: FsmState::Idle,
        }
    }

    pub fn transition(&mut self, next_state: FsmState) {
        if self.current_state != FsmState::Complete && self.current_state != FsmState::Fault {
            self.current_state = next_state;
        }
    }

    pub async fn execute_handle(
        &mut self,
        actions: Vec<AgentAction>,
        _memory: Arc<Mutex<MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
    ) -> Result<Vec<String>, String> {
        self.transition(FsmState::Executing);
        let mut results = Vec::new();

        for action in actions {
            if self.current_state == FsmState::Complete
                || self.current_state == FsmState::Fault
            {
                break;
            }

            match action {
                AgentAction::ExecuteCommand { command } => {
                    let _ = LazarusVault::auto_snapshot("src/", "pre_cmd").await;
                    let output = Command::new("sh")
                        .arg("-c")
                        .arg(&command)
                        .output()
                        .await
                        .map_err(|e| e.to_string())?;
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if !stdout.is_empty() {
                        results.push(stdout);
                    }
                    if !stderr.is_empty() {
                        results.push(format!("STDERR: {}", stderr));
                    }
                }

                AgentAction::GitAction { command, message } => {
                    let msg = message.clone().unwrap_or_default();
                    // Extract the last meaningful word from message as branch/target name
                    let target = msg
                        .split_whitespace()
                        .last()
                        .unwrap_or("nexus-patch")
                        .trim_matches(|c| c == '\'' || c == '"' || c == '.');

                    let git_cmd = match command.as_str() {
                        // "status" — plain git status
                        "status" => "git status".to_string(),
                        // "branch" — create-or-reset branch (safe for new branches)
                        "branch" => format!("git checkout -B {}", target),
                        // "checkout" — switch to existing branch only
                        "checkout" => format!("git checkout {}", target),
                        // "commit" — stage everything and commit
                        "commit" => {
                            format!("git add -A && git commit -m \"{}\"", msg)
                        }
                        // "push" — push current branch to origin
                        "push" => "git push --set-upstream origin HEAD".to_string(),
                        // "diff" — show staged diff
                        "diff" => "git diff --staged".to_string(),
                        // "log" — recent history
                        "log" => "git log --oneline -10".to_string(),
                        _ => "git status".to_string(),
                    };

                    let output = Command::new("sh")
                        .arg("-c")
                        .arg(&git_cmd)
                        .output()
                        .await
                        .map_err(|e| e.to_string())?;

                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    results.push(format!("GIT[{}]: {}", command, stdout));
                    if !stderr.is_empty() {
                        results.push(format!("GIT_STDERR: {}", stderr));
                    }
                }

                AgentAction::WriteFile { path, content } => {
                    // Sandbox: never overwrite protected kernel DNA via this path
                    if path.starts_with("src/kernel") || path == "Cargo.toml" {
                        results.push(format!(
                            "WRITE_BLOCKED: {} is DNA-protected",
                            path
                        ));
                        continue;
                    }
                    tokio::fs::write(&path, &content)
                        .await
                        .map_err(|e| e.to_string())?;
                    results.push(format!("FILE_WRITTEN: {}", path));
                }

                AgentAction::LazarusSnapshot { path, tag } => {
                    let tag_str = tag.as_deref().unwrap_or("auto");
                    match LazarusVault::auto_snapshot(&path, tag_str).await {
                        Ok(snap) => results.push(format!(
                            "SNAPSHOT_OK: {} → {}",
                            snap.path, snap.id
                        )),
                        Err(e) => results.push(format!("SNAPSHOT_FAULT: {}", e)),
                    }
                }

                // ── WASM Forge ─────────────────────────────────────────────
                AgentAction::SynthesizeWasm { code } => {
                    let _ = LazarusVault::auto_snapshot("src/", "pre_forge").await;
                    match crate::wasm_forge::WasmForge::synthesize_cell(&code).await {
                        Ok(b64) => {
                            println!(">> [FORGE] Dispatching KERNEL_GENESIS payload to UI.");
                            results.push("WASM_SYNTHESIS_COMPLETE".into());
                            // FIX: prefix _ws_payload to silence unused-variable warning
                            let _ws_payload = serde_json::json!({
                                "type": "KERNEL_GENESIS",
                                "payload": b64
                            })
                            .to_string();
                            results.push(format!("GENESIS_PAYLOAD:{}", b64));
                        }
                        Err(e) => results.push(format!("FORGE_ERROR: {}", e)),
                    }
                }

                AgentAction::Halt => {
                    results.push("MISSION_HALTED".into());
                    self.transition(FsmState::Complete);
                    break;
                }

                _ => results.push("ACTION_PENDING".into()),
            }
        }

        Ok(results)
    }
}
