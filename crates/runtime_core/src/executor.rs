use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use nexus_substrate::memory::MemoryPalace;
use crate::shared_state::VmoState;
use crate::distributed_scheduler::DistributedScheduler;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Clone)]
pub enum FsmState { Idle, Executing, Complete, Fault }

pub struct RoboticExecutor {
    pub current_state: FsmState,
    pub api_key: String,
    tx: tokio::sync::broadcast::Sender<String>,
}

pub fn sanitize(path: &str) -> Result<PathBuf> {
    // ⚡ Allow /tmp/ prefix, block all other absolute paths
    if (path.starts_with('/') && !path.starts_with("/tmp/")) || path.contains("..") {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {}", path));
    }
    
    let p = Path::new(path);
    if (p.is_absolute() && !path.starts_with("/tmp/")) || p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {}", path));
    }
    Ok(p.to_path_buf())
}

impl RoboticExecutor {
    pub fn new(api_key: String, tx: tokio::sync::broadcast::Sender<String>) -> Self {
        Self { current_state: FsmState::Idle, api_key, tx }
    }

    pub async fn execute_handle(
        &mut self,
        actions: Vec<AgentAction>,
        memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>> {
        self.current_state = FsmState::Executing;
        let mut observations = Vec::new();

        for action in actions {
            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => {
                    let summary = obs.lines().next().unwrap_or("[OK]");
                    let _ = self.tx.send(serde_json::json!({
                        "type": "LOG_EVENT",
                        "payload": format!("[EXECUTOR] Action executed: {}", summary)
                    }).to_string());
                    observations.push(obs);
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

    pub async fn execute_single_action(
        &mut self,
        action: AgentAction,
        _memory: Arc<Mutex<MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        let ledger_action = action.clone();
        let result: Result<String> = async {
        match action {
            AgentAction::ExecuteCommand { command, .. } => {
                let out = crate::podman::PodmanShield::execute(&command)?;
                let mut stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                let mut stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                
                // ⚡ DEFENSE IN DEPTH 1: Truncate raw command output at the source
                if stdout.len() > 8000 { stdout.truncate(8000); stdout.push_str("
...[STDOUT TRUNCATED: OUTPUT TOO LARGE]"); }
                if stderr.len() > 8000 { stderr.truncate(8000); stderr.push_str("
...[STDERR TRUNCATED: OUTPUT TOO LARGE]"); }
                let mut res = format!("[CMD_OK] Status: {}\n", out.status);
                if !stdout.trim().is_empty() { res.push_str(&format!("STDOUT:\n{}\n", stdout)); }
                if !stderr.trim().is_empty() { res.push_str(&format!("STDERR:\n{}\n", stderr)); }
                if stdout.trim().is_empty() && stderr.trim().is_empty() { res.push_str("(Command produced no output)\n"); }
                Ok(res)
            }
            AgentAction::WriteFile { path, content, .. } => {
                let safe_path = sanitize(&path)?;
                tokio::fs::write(&safe_path, content).await?;
                Ok(format!("[WRITE_OK] {}", path))
            }
            AgentAction::ReadFile { path, .. } => {
                let safe_path = sanitize(&path)?;
                let content = tokio::fs::read_to_string(&safe_path).await?;
                
                // ⚡ SOVEREIGN RAG: Vectorize and store in MemoryPalace
                // Use a 8k token safe-chunk for the embedding API
                let snippet = if content.len() > 8000 { &content[..8000] } else { &content };
                let mem = _memory.lock().await;
                if let Ok(emb) = nexus_substrate::vector_bridge::get_embedding(&self.api_key, snippet).await {
                    let _ = mem.store_memory("ReadFile", &path, &content, Some(emb)).await;
                } else {
                    let _ = mem.store_memory("ReadFile", &path, &content, None).await;
                }
                
                // Return a truncated preview to the immediate GoT loop to prevent context overflow
                let return_content = if content.len() > 1500 {
                    format!("{}...\n\n[CONTENT TRUNCATED. FULL DATA SAVED TO MEMORY PALACE VECTOR VAULT. USE search_memory TO EXPLORE DEEPER.]", &content[..1500])
                } else {
                    content.clone()
                };
                
                Ok(format!("[READ_OK] {}\nCONTENT PREVIEW:\n{}", path, return_content))
            }
            AgentAction::ReadFunction { path, function_name, .. } => {
                let safe_path = sanitize(&path)?;
                let content = tokio::fs::read_to_string(&safe_path).await?;
                Ok(format!("[READ_FUNCTION_OK] '{}' from {}\nCONTENT:\n{}", function_name, path, content))
            }
            AgentAction::DiffPatch { path, search, replace, .. } => {
                let safe_path = sanitize(&path)?;
                let original = tokio::fs::read_to_string(&safe_path).await?;
                let patched = crate::diff::DiffEngine::apply(&original, &search, &replace)?;
                tokio::fs::write(&safe_path, patched).await?;
                Ok(format!("[DIFF_PATCH_OK] Applied changes to {}", path))
            }
            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                let safe_path = sanitize(&path)?;
                let patched = crate::ast_editor::AstEditor::update_function(safe_path.to_str().unwrap_or(&path), &function_name, &new_body)?;
                tokio::fs::write(&safe_path, patched).await?;
                Ok(format!("[PATCH_FUNCTION_OK] Updated function '{}' in {}", function_name, path))
            }
            AgentAction::SearchMemory { query, .. } => {
                let mem = _memory.lock().await;
                let mut results_text = String::new();
                
                // 1. Semantic Vector Search
                if let Ok(emb) = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &query).await {
                    if let Ok(results) = mem.semantic_search(&emb).await {
                        for r in results {
                            let snippet = if r.result.len() > 3000 { &r.result[..3000] } else { &r.result };
                            results_text.push_str(&format!("Source: {}\nData: {}...\n---\n", r.directive, snippet));
                        }
                    }
                }
                
                // 2. Fallback to FTS5 Lexical Search if Vector fails or is empty
                if results_text.is_empty() {
                    if let Ok(results) = mem.search_memories(&query).await {
                        for r in results {
                            let snippet = if r.result.len() > 3000 { &r.result[..3000] } else { &r.result };
                            results_text.push_str(&format!("Source: {}\nData: {}...\n---\n", r.directive, snippet));
                        }
                    }
                }
                
                if results_text.is_empty() { 
                    Ok(format!("[SEARCH_OK] No memories found in VectorVault for: {}", query))
                } else { 
                    Ok(format!("[SEARCH_OK] Found semantic context:\n{}", results_text))
                }
            }
            AgentAction::VmoCreate { size, .. } => {
                let handle = nexus_substrate::kernel::safe_kernel::SafeKernel::create_vmo(size)
                    .map_err(|e| anyhow!("VMO Allocation Fault: {:?}", e))?;
                Ok(format!("[VMO_CREATED] Handle: {}", handle))
            }
            AgentAction::CalculateEntropy { handle, .. } => {
                let h_val = handle.parse::<u32>().unwrap_or(0);
                let entropy = nexus_substrate::kernel::calculate_entropy(h_val);
                Ok(format!("[ENTROPY] Handle {}: {:.4} Shannon bits", handle, entropy))
            }
            AgentAction::LazarusSnapshot { path, tag, .. } => {
                let safe_path = sanitize(&path)?;
                let directive = tag.unwrap_or_else(|| "Manual Snapshot".into());
                let snap = crate::lazarus_vault::take_snapshot(safe_path.to_str().unwrap(), &directive).await?;
                Ok(format!("[SNAPSHOT_OK] ID: {} Hash: {}", snap.id, snap.hash))
            }
            AgentAction::ExportReport { title, content, .. } => {
                let file_name = format!("{}.md", title.replace(" ", "_").to_lowercase());
                let safe_path = sanitize(&file_name)?;
                tokio::fs::write(&safe_path, content).await?;
                Ok(format!("[REPORT_EXPORTED] {}", file_name))
            }
            AgentAction::FetchUiSnapshot { .. } => {
                let content = tokio::fs::read_to_string("ui/src/App.svelte").await.unwrap_or_else(|_| "UI not found".into());
                Ok(format!("[UI_SNAPSHOT] Fetched {} bytes from App.svelte", content.len()))
            }
            AgentAction::ScrapeUrl { url, .. } => {
                let client = reqwest::Client::new();
                let res = client.post("http://127.0.0.1:5000/scrape")
                    .json(&serde_json::json!({ "url": url }))
                    .send()
                    .await?;
                let text = res.text().await?;
                Ok(format!("[URL_SCRAPED] Output: {}", text))
            }
            AgentAction::GitRollback { reasoning } => {
                let out = crate::podman::PodmanShield::execute("git reset --hard HEAD")?;
                Ok(format!("[GIT_ROLLBACK_OK] Reason: {}\nSTDOUT:\n{}", reasoning, String::from_utf8_lossy(&out.stdout)))
            }
            AgentAction::SpawnSubagent { instruction, .. } | AgentAction::DelegateTask { instruction, .. } => {
                let mem = _memory.lock().await;
                mem.store_memory("Executor", &instruction, "Delegated to Subagent", None).await?;
                Ok("[DELEGATION_LOGGED] Instruction pushed to Memory Palace for async execution.".to_string())
            }
            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
        }
        }.await;

        if result.is_ok() {
            crate::governance::protocol::GOVERNANCE.lock().await.record(ledger_action, "robotic_executor");
        }

        result
    }
}
