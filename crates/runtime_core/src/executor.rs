// src/executor.rs — v2.7 [IRONCLAD_FSM + AGENTIC_LRL]

use crate::agent_protocol::AgentAction;
use crate::arbiter_agent::ArbiterAgent;
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

            if matches!(action, AgentAction::Halt { .. }) {
            }

            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => {
                    observations.push(obs.clone());
                    tracing::info!("Action observation: {}", obs);

                    if obs.contains("[READ_OK]") || obs.contains("[WRITE_OK]") || obs.contains("[CMD:") || obs.contains("[DIFF_PATCH_OK]") || obs.contains("[PATCH_OK]") || obs.contains("[REPORT_OK]") || obs.contains("[HALT]") {
                        let skipped = total.saturating_sub(idx + 1);
                        if skipped > 0 {
                            let yield_msg = format!(">> [YIELD] Action completed. {} action(s) suspended for next turn.", skipped);
                            observations.push(yield_msg.clone());
                            tracing::info!("{}", yield_msg);
                        }
                        break;
                    }
                }
                Err(e) => {
                    let err_obs = format!("ACTION_FAILED: {}", e);
                    observations.push(err_obs.clone());
                    tracing::warn!("Action failed: {}", e);
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
            AgentAction::ExecuteCommand { command, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Reasoning: {}", reasoning)}).to_string());
                if let Err(e) = ArbiterAgent::validate_action("execute_command", &command) {
                    return Ok(format!("SHIELD_BLOCKED: {}", e));
                }
                
                // ⚡ LRL FUSION: Route strictly through AgenticRuntime
                let mut lrl = crate::agents::agentic_runtime::AgenticRuntime::new(
                    crate::agents::agentic_runtime::SubstrateContext {
                        memory: _memory.clone(),
                        vmo_state: _vmo_state.clone(),
                        dist: _dist.clone(),
                    }
                );
                
                // Inject the ecosystem
                lrl.register_agent(Box::new(crate::agents::scout_agent::ScoutAgent::new()));
                lrl.register_agent(Box::new(crate::agents::shell_agent::ShellAgent));
                
                match lrl.dispatch(&command).await {
                    Ok(output) => Ok(format!("[CMD:LRL_ROUTED]\n{}\nReason: {}", output, reasoning)),
                    Err(e) => Ok(format!("LRL_FAULT: {}", e)),
                }
            }

            AgentAction::WriteFile { path, content, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Writing file: {} | Reason: {}", path, reasoning)}).to_string());
                let content_len = content.len();
                let preview: String = content.chars().take(80).collect();
                tokio::fs::write(&path, content).await
                    .map(|_| format!("[WRITE_OK] {} bytes → {} | Preview: {}…", content_len, path, preview))
                    .map_err(|e| anyhow::anyhow!("WRITE_FAILED: {}", e))
            }

            AgentAction::ReadFile { path, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Reading file: {} | Reason: {}", path, reasoning)}).to_string());
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        let byte_len = content.len();
                        const MAX_CHARS: usize = 4096;
                        let char_count = content.chars().count();
                        let injected: String = content.chars().take(MAX_CHARS).collect();
                        let note = if char_count > MAX_CHARS {
                            format!("\n[TRUNCATED: showing {}/{} chars — use diff_patch for large files]", MAX_CHARS, char_count)
                        } else { String::new() };
                        Ok(format!("[READ_OK] {} ({} bytes){}\n--- CONTENT ---\n{}\n--- END CONTENT ---", path, byte_len, note, injected))
                    }
                    Err(e) => anyhow::bail!("READ_FAILED: {}", e),
                }
            }

            AgentAction::DiffPatch { path, search, replace, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Applying diff patch to {} | Reason: {}", path, reasoning)}).to_string());
                match tokio::fs::read_to_string(&path).await {
                    Ok(original) => {
                        let orig_lines = original.lines().count();
                        match crate::diff::DiffEngine::apply(&original, &search, &replace) {
                            Ok(new_content) => {
                                let new_lines: usize = new_content.lines().count();
                                match tokio::fs::write(&path, &new_content).await {
                                    Ok(_) => Ok(format!("[DIFF_PATCH_OK] {} | Lines: {} → {}", path, orig_lines, new_lines)),
                                    Err(e) => Ok(format!("[DIFF_PATCH_WRITE_FAILED] {}", e)),
                                }
                            }
                            Err(e) => Ok(format!("[DIFF_PATCH_FAILED] {}", e)),
                        }
                    }
                    Err(e) => Ok(format!("[DIFF_PATCH_READ_FAILED] {}", e)),
                }
            }

            AgentAction::PatchFunction { path, function_name, new_body, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Patching function {} in {} | Reason: {}", function_name, path, reasoning)}).to_string());
                match crate::ast_editor::AstEditor::update_function(&path, &function_name, &new_body) {
                    Ok(full_code) => {
                        match tokio::fs::write(&path, full_code).await {
                            Ok(_) => Ok(format!("[PATCH_OK] Function '{}' updated in {}", function_name, path)),
                            Err(e) => Ok(format!("PATCH_WRITE_FAILED: {}", e)),
                        }
                    },
                    Err(e) => Ok(format!("PATCH_FAILED: {}", e)),
                }
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
                let clean_reason = reasoning.chars().take(100).collect::<String>();
                let _ = self.tx.send(serde_json::json!({"type": "TASK_STATUS", "payload": format!("HALT: {}", clean_reason)}).to_string());
                Ok(format!("[HALT] {}", clean_reason))
            }

            _ => Ok(format!("[STUB_OK] Action acknowledged: {:?}", action))
        }
    }
}
