// src/executor.rs — v2.8 [CLEANED + COMPLIANT]
use crate::agent_protocol::AgentAction;
use crate::distributed_scheduler::DistributedScheduler;
use crate::shared_state::VmoState;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

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
        &mut self,
        actions: Vec<AgentAction>,
        memory: Arc<Mutex<nexus_substrate::memory::MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>> {
        self.current_state = FsmState::Executing;
        let mut observations = Vec::new();
        let total = actions.len();

        for (idx, action) in actions.into_iter().enumerate() {
            let _ = self.tx.send(serde_json::json!({
                "type": "TASK_STATUS",
                "payload": format!("Executing → {:?}", action)
            }).to_string());

            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => {
                    observations.push(obs.clone());
                    tracing::info!("Action observation: {}", obs);
                    if obs.contains("[READ_OK]") || obs.contains("[WRITE_OK]") || obs.contains("[CMD:") ||
                       obs.contains("[DIFF_PATCH_OK]") || obs.contains("[PATCH_OK]") ||
                       obs.contains("[REPORT_OK]") || obs.contains("[HALT]") {
                        let skipped = total.saturating_sub(idx + 1);
                        if skipped > 0 {
                            observations.push(format!(">> [YIELD] {} actions suspended.", skipped));
                        }
                        break;
                    }
                }
                Err(e) => {
                    observations.push(format!("ACTION_FAILED: {}", e));
                    self.current_state = FsmState::Fault;
                    return Ok(observations);
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
            AgentAction::VmoCreate { size, reasoning: _ } => {
                let handle = unsafe { nexus_substrate::kernel::ffi::zx_vmo_create_native(size) };
                if handle == 0 { Ok("[VMO_FAIL]".into()) }
                else { Ok(format!("[VMO_OK] 0x{:x}", handle)) }
            }
            AgentAction::ReadFile { path, reasoning: _ } => {
                let content = tokio::fs::read_to_string(&path).await.map_err(|e| anyhow::anyhow!(e))?;
                Ok(format!("[READ_OK] {} ({} bytes)", path, content.len()))
            }
            AgentAction::Halt { reasoning: _ } => {
                self.current_state = FsmState::Complete;
                Ok("[HALT]".into())
            }
            _ => Ok("[STUB_OK] Action accepted".into())
        }
    }
}
