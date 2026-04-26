// src/executor.rs — v8.0 [PRODUCTION_CAUSAL]
use crate::agent_protocol::AgentAction;
use crate::distributed_scheduler::DistributedScheduler;
use crate::memory::MemoryPalace;
use crate::server::VmoState;
use crate::lazarus_vault::LazarusVault;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn, error};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FsmState { Idle, Executing, Complete, Fault }

pub struct RoboticExecutor {
    pub current_state: FsmState,
    pub api_key: String,
    pub tx: broadcast::Sender<String>,
}

impl RoboticExecutor {
    pub fn new(api_key: String, tx: broadcast::Sender<String>) -> Self {
        Self {
            current_state: FsmState::Idle,
            api_key,
            tx,
        }
    }

    pub fn transition(&mut self, next: FsmState) {
        if self.current_state != FsmState::Complete && self.current_state != FsmState::Fault {
            info!(from = ?self.current_state, to = ?next, "Executor State Transition");
            self.current_state = next;
        }
    }

    pub async fn execute_handle(
        &mut self,
        actions: Vec<AgentAction>,
        _memory: Arc<Mutex<MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>, String> {
        self.transition(FsmState::Executing);
        let mut results = Vec::new();

        for action in actions {
            match action {
                AgentAction::WriteFile { path, content } => {
                    info!(path = %path, "Auto-snapshotting before production write");
                    // ⚡ IMMUNE SYSTEM: Snapshot before every write
                    let _ = LazarusVault::auto_snapshot(&path, "pre_write_v19").await;
                    
                    if std::fs::write(&path, &content).is_ok() { 
                        results.push(format!("OK: {}", path)); 
                    } else {
                        error!("Failed to commit write to {}", path);
                        results.push(format!("FAIL: {}", path));
                    }
                }
                AgentAction::ExecuteCommand { command } => {
                    if let Err(e) = crate::arbiter_agent::ArbiterAgent::validate_action("shell", &command) {
                        warn!("Arbiter vetoed command: {}", command);
                        results.push(format!("BLOCKED: {}", e));
                        continue;
                    }
                    let out = Command::new("sh").arg("-c").arg(&command).output().await.map_err(|e| e.to_string())?;
                    results.push(format!("OUT: {}", String::from_utf8_lossy(&out.stdout)));
                }
                AgentAction::Halt => { 
                    self.transition(FsmState::Complete); 
                    break; 
                }
                _ => {
                    warn!("Action variants currently pending logic implementation.");
                    results.push("ACTION_PENDING".into());
                }
            }
        }
        Ok(results)
    }
}
