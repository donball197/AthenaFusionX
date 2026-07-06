// src/executor.rs — v4.7 Precise DevOps Executor
use crate::agent_protocol::AgentAction;
use crate::memory::MemoryPalace;
use crate::server::VmoState;
use crate::lazarus_vault::LazarusVault;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::process::Command;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FsmState { Idle, Executing, Complete, Fault }

pub struct RoboticExecutor { pub current_state: FsmState }

impl RoboticExecutor {
    pub fn new() -> Self { Self { current_state: FsmState::Idle } }

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
            if self.current_state == FsmState::Complete || self.current_state == FsmState::Fault { break; }

            match action {
                AgentAction::ExecuteCommand { command } => {
                    let _ = LazarusVault::auto_snapshot("src/", "pre_cmd").await;
                    let output = Command::new("sh").arg("-c").arg(&command).output().await.map_err(|e| e.to_string())?;
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if !stdout.is_empty() { results.push(stdout); }
                    if !stderr.is_empty() { results.push(format!("STDERR: {}", stderr)); }
                }
                AgentAction::GitAction { command, message } => {
                    let msg = message.unwrap_or_default();
                    let clean_val = msg.split_whitespace().last().unwrap_or("nexus-patch").trim_matches(|c| c == '\'' || c == '"' || c == '.');
                    let git_cmd = match command.as_str() {
                        "status" => "git status".to_string(),
                        "branch" => format!("git checkout -B {}", clean_val),
                        "commit" => format!("git add . && git commit -m \"{}\"", msg),
                        "checkout" => format!("git checkout {}", clean_val),
                        _ => "git status".to_string(),
                    };
                    let output = Command::new("sh").arg("-c").arg(&git_cmd).output().await.map_err(|e| e.to_string())?;
                    results.push(format!("GIT: {}", String::from_utf8_lossy(&output.stdout).trim()));
                }
                AgentAction::WriteFile { path, content } => {
                    tokio::fs::write(&path, &content).await.map_err(|e| e.to_string())?;
                    results.push(format!("FILE_WRITTEN: {}", path));
                }
                // ⚡ V14: The WASM Forge Hook
                AgentAction::SynthesizeWasm { code } => {
                    let _ = LazarusVault::auto_snapshot("src/", "pre_forge").await;
                    match crate::wasm_forge::WasmForge::synthesize_cell(&code).await {
                        Ok(b64) => {
                            println!(">> [FORGE] Dispatching KERNEL_GENESIS payload to UI.");
                            results.push("WASM_SYNTHESIS_COMPLETE".into());

                            // ⚡ V14: Direct WebSocket push using the global TX channel
                            // We construct a specific KERNEL_GENESIS message to bypass log parsing.
                            let ws_payload = serde_json::json!({
                                "type": "KERNEL_GENESIS",
                                "payload": b64
                            }).to_string();

                            // We have to route this through the active FSM transaction.
                            // The easiest way is to just push it as a raw JSON string to results,
                            // and the `task_manager.rs` loop will forward it if we format it right.
                            // Actually, let's inject a special "GENESIS_PAYLOAD" tag the UI knows to parse.
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
