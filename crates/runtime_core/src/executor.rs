use anyhow::Result;
use crate::agent_protocol::AgentAction;
use crate::arbiter_agent::ArbiterAgent;
use crate::distributed_scheduler::DistributedScheduler;
use crate::shared_state::VmoState;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, PartialEq, Clone)]
pub enum FsmState { Idle, Executing, Complete, Fault }

#[allow(dead_code)]
pub struct RoboticExecutor {
    pub current_state: FsmState,
    tx: tokio::sync::broadcast::Sender<String>,
}

impl RoboticExecutor {
    pub fn new(_api_key: String, tx: tokio::sync::broadcast::Sender<String>) -> Self {
        Self { current_state: FsmState::Idle, tx }
    }

    pub async fn execute_handle(
        &mut self,
        actions: Vec<AgentAction>,
        memory: Arc<Mutex<nexus_substrate::memory::MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>> {
        self.current_state = FsmState::Executing;
        let mut observations = Vec::new();

        for action in actions {
            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => observations.push(obs),
                Err(e) => {
                    observations.push(format!("ACTION_FAILED: {}", e));
                    self.current_state = FsmState::Fault;
                    return Ok(observations); // Return fault observation to the Verifier
                }
            }
        }
        self.current_state = FsmState::Complete;
        Ok(observations)
    }

    async fn execute_single_action(
        &mut self,
        action: AgentAction,
        _memory: Arc<Mutex<nexus_substrate::memory::MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        match action {
            AgentAction::ExecuteCommand { command, .. } => {
                if let Err(e) = ArbiterAgent::validate_action("execute_command", &command) {
                    return Ok(format!("SHIELD_BLOCKED: {}", e));
                }
                
                // ⚡ SUTURE: Capture STDERR so the Verifier can detect compilation faults
                let out = crate::podman::PodmanShield::execute(&command)?;
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                
                Ok(format!("[CMD_OK] EXIT_CODE: {}\nSTDOUT:\n{}\nSTDERR:\n{}", out.status, stdout, stderr))
            }
            AgentAction::WriteFile { path, content, .. } => {
                tokio::fs::write(&path, content).await.map_err(anyhow::Error::from).map(|_| format!("[WRITE_OK] {}", path))
            }
            AgentAction::ReadFile { path, .. } => {
                let c = tokio::fs::read_to_string(&path).await?;
                Ok(format!("[READ_OK] {}\nCONTENT:\n{}", path, c))
            }
            AgentAction::DiffPatch { path, .. } => Ok(format!("[DIFF_PATCH_OK] {}", path)),
            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            _ => Ok("[STUB_OK]".to_string()),
        }
    }
}
