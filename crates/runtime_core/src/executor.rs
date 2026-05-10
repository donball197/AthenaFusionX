// src/executor.rs — v2.8 [STUB_GUARD + MEMORY_WIRED]

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
    pub api_key: String,
}

impl RoboticExecutor {
    pub fn new(_api_key: String, tx: tokio::sync::broadcast::Sender<String>) -> Self {
        Self { current_state: FsmState::Idle, tx, api_key: _api_key }
    }

    pub async fn execute_handle(
        &mut self,
        actions:   Vec<AgentAction>,
        memory:    Arc<Mutex<nexus_substrate::memory::MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist:      Arc<DistributedScheduler>,
    ) -> Result<Vec<String>> {
        self.current_state = FsmState::Executing;
        let mut observations = Vec::new();
        let total = actions.len();

        for (idx, action) in actions.into_iter().enumerate() {
            let _ = self.tx.send(serde_json::json!({
                "type":    "TASK_STATUS",
                "payload": format!("Executing → {:?}", action)
            }).to_string());

            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => {
                    observations.push(obs.clone());
                    tracing::info!("Action observation: {}", obs);

                    let is_terminal = obs.contains("[READ_OK]")
                        || obs.contains("[WRITE_OK]")
                        || obs.contains("[CMD:")
                        || obs.contains("[DIFF_PATCH_OK]")
                        || obs.contains("[PATCH_OK]")
                        || obs.contains("[REPORT_OK]")
                        || obs.contains("[HALT]")
                        || obs.contains("[MEMORY_OK]");

                    if is_terminal {
                        let skipped = total.saturating_sub(idx + 1);
                        if skipped > 0 {
                            observations.push(format!(
                                ">> [YIELD] Action completed. {} action(s) suspended.", skipped
                            ));
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
        action:    AgentAction,
        memory:    Arc<Mutex<nexus_substrate::memory::MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist:      Arc<DistributedScheduler>,
    ) -> Result<String> {
        match action {
            AgentAction::ExecuteCommand { command, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Reasoning: {}", reasoning)}).to_string());
                if let Err(e) = ArbiterAgent::validate_action("execute_command", &command) {
                    return Ok(format!("SHIELD_BLOCKED: {}", e));
                }
                let mut lrl = crate::agents::agentic_runtime::AgenticRuntime::new(
                    crate::agents::agentic_runtime::SubstrateContext {
                        memory: memory.clone(),
                        vmo_state: vmo_state.clone(),
                        dist: dist.clone(),
                    }
                );
                lrl.register_agent(Box::new(crate::agents::scout_agent::ScoutAgent::new()));
                lrl.register_agent(Box::new(crate::agents::shell_agent::ShellAgent));
                match lrl.dispatch(&command).await {
                    Ok(out) => Ok(format!("[CMD:LRL_ROUTED]\n{}\nReason: {}", out, reasoning)),
                    Err(e)  => Ok(format!("LRL_FAULT: {}", e)),
                }
            }

            AgentAction::WriteFile { path, content, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Writing: {} | {}", path, reasoning)}).to_string());
                // Guard: prevent relative paths from landing in unexpected CWD locations.
                if path.contains("..") || std::path::Path::new(&path).is_absolute() {
                    return Ok(format!("[WRITE_BLOCKED] Path traversal or absolute path rejected: {}", path));
                }
                let content_len = content.len();
                let preview: String = content.chars().take(80).collect();
                tokio::fs::write(&path, content).await
                    .map(|_| format!("[WRITE_OK] {} bytes → {} | Preview: {}…", content_len, path, preview))
                    .map_err(|e| anyhow::anyhow!("WRITE_FAILED: {}", e))
            }

            AgentAction::ReadFile { path, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Reading: {} | {}", path, reasoning)}).to_string());
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        const MAX_CHARS: usize = 4096;
                        let char_count = content.chars().count();
                        let injected: String = content.chars().take(MAX_CHARS).collect();
                        let note = if char_count > MAX_CHARS {
                            format!("\n[TRUNCATED: showing {}/{} chars]", MAX_CHARS, char_count)
                        } else { String::new() };
                        Ok(format!("[READ_OK] {} ({} bytes){}\n--- CONTENT ---\n{}\n--- END CONTENT ---",
                            path, content.len(), note, injected))
                    }
                    Err(e) => anyhow::bail!("READ_FAILED: {}", e),
                }
            }

            AgentAction::DiffPatch { path, search, replace, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Patching: {} | {}", path, reasoning)}).to_string());
                match tokio::fs::read_to_string(&path).await {
                    Ok(original) => {
                        let orig_lines = original.lines().count();
                        match crate::diff::DiffEngine::apply(&original, &search, &replace) {
                            Ok(new_content) => {
                                let new_lines = new_content.lines().count();
                                match tokio::fs::write(&path, &new_content).await {
                                    Ok(_)  => Ok(format!("[DIFF_PATCH_OK] {} | Lines: {} → {}", path, orig_lines, new_lines)),
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
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Patching fn {} in {} | {}", function_name, path, reasoning)}).to_string());
                match crate::ast_editor::AstEditor::update_function(&path, &function_name, &new_body) {
                    Ok(full_code) => match tokio::fs::write(&path, full_code).await {
                        Ok(_)  => Ok(format!("[PATCH_OK] Function '{}' updated in {}", function_name, path)),
                        Err(e) => Ok(format!("PATCH_WRITE_FAILED: {}", e)),
                    },
                    Err(e) => Ok(format!("PATCH_FAILED: {}", e)),
                }
            }

            AgentAction::SearchMemory { query, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Searching memory: {} | {}", query, reasoning)}).to_string());
                // Wire directly to the real MemoryPalace FTS engine.
                let mem = memory.lock().await;
                match mem.search_memories(&query).await {
                    Ok(results) if results.is_empty() => {
                        Ok(format!("[MEMORY_OK] Search for '{}' returned 0 results. \
                            Memory palace may be empty or no match found. \
                            Do NOT fabricate data — proceed with what is known or halt.", query))
                    }
                    Ok(results) => {
                        let summary: Vec<String> = results.iter().map(|m| {
                            format!("  [{}] origin={} | directive={} | result={}",
                                m.timestamp, m.origin,
                                m.directive.chars().take(60).collect::<String>(),
                                m.result.chars().take(80).collect::<String>())
                        }).collect();
                        Ok(format!("[MEMORY_OK] {} result(s) for '{}':\n{}", results.len(), query, summary.join("\n")))
                    }
                    Err(e) => Ok(format!("[MEMORY_FAULT] search_memories failed: {}", e)),
                }
            }

            AgentAction::ExportReport { title, content, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Exporting: {} | {}", title, reasoning)}).to_string());
                let safe_name = title.to_lowercase()
                    .replace(|c: char| !c.is_alphanumeric() && c != '-', "_");
                let path = format!("src/{}.md", safe_name);
                tokio::fs::write(&path, content).await
                    .map(|_| format!("[REPORT_OK] Exported → {}", path))
                    .map_err(|e| anyhow::anyhow!("EXPORT_FAILED: {}", e))
            }

            AgentAction::FetchUiSnapshot { reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"LOG_EVENT","payload":format!("fetch_ui_snapshot — {}", reasoning)}).to_string());
                Ok("[UI_SNAPSHOT_REQUESTED] Ghost DOM state capture requested.".to_string())
            }

            
            AgentAction::DelegateTask { ref instruction, ref tier, token_limit, reasoning: _ } => {
                let model_tier = if tier.to_lowercase().contains("matriarch") {
                    crate::agent_protocol::ModelTier::Matriarch
                } else {
                    crate::agent_protocol::ModelTier::Specialist
                };
                let orch = crate::orchestrator::Orchestrator::new();
                                let result = orch.run_subagent(instruction.clone(), model_tier, token_limit, &self.api_key).await;
                Ok(crate::logic_engine::LogicEngine::linearize_handoff(&action, result))
            }

            AgentAction::Halt { reasoning } => {
                self.current_state = FsmState::Complete;
                let clean = reasoning.chars().take(100).collect::<String>();
                let _ = self.tx.send(serde_json::json!({"type":"TASK_STATUS","payload":format!("HALT: {}", clean)}).to_string());
                Ok(format!("[HALT] {}", clean))
            }

            // STUB GUARD: Any unimplemented action now returns a clear
            // "no data" signal so the agent cannot hallucinate results.
            
            AgentAction::ScrapeUrl { url, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Scraping: {} | {}", url, reasoning)}).to_string());
                match reqwest::get(&url).await {
                    Ok(resp) => {
                        match resp.text().await {
                            Ok(text) => {
                                // ⚡ Naive HTML strip to preserve LLM context tokens
                                let clean = text.replace("<", " <").split('<').map(|s| s.split('>').next_back().unwrap_or("")).collect::<String>();
                                let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
                                let char_count = clean.chars().count();
                                const MAX_CHARS: usize = 4096;
                                let injected: String = clean.chars().take(MAX_CHARS).collect();
                                let note = if char_count > MAX_CHARS {
                                    format!("\n[TRUNCATED: showing {}/{} chars]", MAX_CHARS, char_count)
                                } else { String::new() };
                                Ok(format!("[SCRAPE_OK] {}{}\n--- CONTENT ---\n{}\n--- END CONTENT ---", url, note, injected))
                            }
                            Err(e) => Ok(format!("SCRAPE_TEXT_FAILED: {}", e)),
                        }
                    }
                    Err(e) => Ok(format!("SCRAPE_NETWORK_FAILED: {}", e)),
                }
            }
            unimplemented_action => {
                let action_name = format!("{:?}", unimplemented_action)
                    .split_whitespace().next().unwrap_or("Unknown").to_string();
                tracing::warn!("[EXECUTOR] Unimplemented action: {}", action_name);
                Ok(format!(
                    "[NOT_IMPLEMENTED] '{}' is not yet implemented and returned NO data. \
                     Do NOT assume success. Do NOT fabricate results. \
                     Either use an alternative action or halt with an honest status.",
                    action_name
                ))
            }
        }
    }
}
