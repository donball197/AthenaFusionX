// src/executor.rs — v4.3 Full Lifecycle DevOps Executor
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
                    // ⚡ SAFETY REFLEX: Lazarus Snapshot before any shell command
                    let _ = LazarusVault::auto_snapshot("src/", "pre_execution_safety").await;

                    let output = Command::new("sh").arg("-c").arg(&command).output().await.map_err(|e| e.to_string())?;
                    let combined = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
                    results.push(combined.trim().to_string());
                }

                AgentAction::GitAction { command, message } => {
                    let msg = message.unwrap_or_default();
                    let clean_val = msg.split_whitespace().next().unwrap_or("nexus-patch");
                    
                    let git_cmd = match command.as_str() {
                        "status" => "git status".to_string(),
                        "branch" => format!("git checkout -b {}", clean_val),
                        "commit" => format!("git add . && git commit -m \"{}\"", msg),
                        "checkout" => format!("git checkout {}", clean_val),
                        _ => "git status".to_string(),
                    };
                    
                    let output = Command::new("sh").arg("-c").arg(&git_cmd).output().await.map_err(|e| e.to_string())?;
                    results.push(format!("GIT_RESULT: {}", String::from_utf8_lossy(&output.stdout).trim()));
                }

                AgentAction::WriteFile { path, content } => {
                    tokio::fs::write(&path, &content).await.map_err(|e| e.to_string())?;
                    results.push(format!("FILE_WRITTEN: {}", path));
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
