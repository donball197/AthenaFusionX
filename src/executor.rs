// src/executor.rs — v8.1 [SEARCH_RESTORED]
use crate::agent_protocol::AgentAction;
use crate::distributed_scheduler::DistributedScheduler;
use crate::genesis_broker;
use crate::lazarus_vault::LazarusVault;
use crate::memory::MemoryPalace;
use crate::server::VmoState;
use crate::wasm_registry::WasmRegistry;
use crate::wasm_runtime::WasmRuntime;
use crate::vision;
use base64::{engine::general_purpose, Engine as _};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FsmState { Idle, Executing, Complete, Fault }

pub struct RoboticExecutor {
    pub current_state:    FsmState,
    pub unverified_writes: bool,
    pub stored_genesis:   Option<String>,
    pub last_func_hint:   Option<String>,
    pub api_key:          String,
    pub tx:               broadcast::Sender<String>,
}

impl RoboticExecutor {
    pub fn new(api_key: String, tx: broadcast::Sender<String>) -> Self {
        Self {
            current_state: FsmState::Idle,
            unverified_writes: false,
            stored_genesis: None,
            last_func_hint: None,
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
        actions:   Vec<AgentAction>,
        memory:    Arc<Mutex<MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
        dist:      Arc<DistributedScheduler>,
    ) -> anyhow::Result<Vec<String>> {
        self.transition(FsmState::Executing);
        let mut raw_results: Vec<String> = Vec::new();

        for action in actions {
            if self.current_state == FsmState::Complete || self.current_state == FsmState::Fault { 
                break; 
            }

            match action {
                AgentAction::SearchMemory { query } => {
                    let q_vec = crate::vector_bridge::get_embedding(&self.api_key, &query).await.unwrap_or_default();
                    let palace = memory.lock().await;
                    if !q_vec.is_empty() {
                        match palace.semantic_search(&q_vec).await {
                            Ok(mems) => {
                                let hits: Vec<String> = mems.iter().map(|m| format!("[MEM #{}] {}", m.id, m.result)).collect();
                                raw_results.push(format!("SEMANTIC_SEARCH_RESULTS: {}", hits.join(" | ")));
                            }
                            Err(e) => error!("SEARCH_FAULT: {}", e),
                        }
                    } else {
                        match palace.search_memories(&query).await {
                            Ok(mems) => {
                                let hits: Vec<String> = mems.iter().map(|m| format!("[MEM #{}] {}", m.id, m.result)).collect();
                                raw_results.push(format!("KEYWORD_SEARCH_RESULTS: {}", hits.join(" | ")));
                            }
                            Err(e) => error!("SEARCH_FAULT: {}", e),
                        }
                    }
                }

                AgentAction::AnalyzeImage { path, query } => {
                    match vision::scan_local_image_cloud(&path, &query, &self.api_key).await {
                        Ok(analysis) => raw_results.push(format!("IMAGE_ANALYSIS_RESULT:\n{}", analysis)),
                        Err(e) => error!("VISION_FAULT: {}", e),
                    }
                }

                AgentAction::WasmLoad { name, func_name, input } => {
                    let reg = WasmRegistry::new(".nexus_wasm_registry.db").await?;
                    if let Ok(Some(cell)) = reg.get(&name).await {
                        let bytes = general_purpose::STANDARD.decode(&cell.wasm_b64)?;
                        match WasmRuntime::run_bytes(&bytes, &func_name, input) {
                            Ok(res) => raw_results.push(format!("WASM_EXEC_OK: {}", res)),
                            Err(e) => error!("WASM_EXEC_FAULT: {}", e),
                        }
                    }
                }

                AgentAction::WasmExecuteLocal { func_name, input } => {
                    if let Some(b64) = &self.stored_genesis {
                        let bytes = general_purpose::STANDARD.decode(b64)?;
                        match WasmRuntime::run_bytes(&bytes, &func_name, input) {
                            Ok(res) => raw_results.push(format!("WASM_LOCAL_EXEC_OK: {}", res)),
                            Err(e) => error!("WASM_EXEC_FAULT: {}", e),
                        }
                    }
                }

                AgentAction::SynthesizeWasm { code } => {
                    match dist.synthesize(&code).await {
                        Ok(b64) => {
                            self.stored_genesis = Some(b64.clone());
                            raw_results.push(format!("GENESIS_PAYLOAD:{}", b64));
                        }
                        Err(e) => error!("FORGE_ERROR: {}", e),
                    }
                }

                AgentAction::WriteFile { path, content } => {
                    if let Ok(safe_path) = crate::sandbox::validate_path(&path) {
                        if std::fs::write(&safe_path, &content).is_ok() {
                            raw_results.push(format!("WRITE_OK: {}", safe_path.display()));
                        }
                    } else {
                        self.transition(FsmState::Fault);
                    }
                }

                AgentAction::ReadFile { path } => {
                    if let Ok(safe_path) = crate::sandbox::validate_path(&path) {
                        if let Ok(content) = std::fs::read_to_string(&safe_path) {
                            raw_results.push(format!("FILE_CONTENT:\n{}", content));
                        }
                    }
                }

                AgentAction::ExecuteCommand { command } => {
                    let out = Command::new("sh").arg("-c").arg(&command).output().await?;
                    raw_results.push(format!("CMD_OUT: {}", String::from_utf8_lossy(&out.stdout)));
                }

                AgentAction::LazarusSnapshot { path, tag } => {
                    let t = tag.as_deref().unwrap_or("auto");
                    if let Ok(snap) = LazarusVault::auto_snapshot(&path, t).await {
                        raw_results.push(format!("SNAPSHOT_OK: {}", snap.id));
                    }
                }

                AgentAction::Halt => {
                    self.transition(FsmState::Complete);
                    break;
                }

                _ => { raw_results.push("ACTION_ACKNOWLEDGED".into()); }
            }
        }

        let (clean, b64_opt) = genesis_broker::extract(&raw_results);
        if let Some(b64) = b64_opt { self.stored_genesis = Some(b64); }
        Ok(clean)
    }
}
