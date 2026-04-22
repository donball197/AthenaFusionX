// src/executor.rs — v4.9 Full-Spectrum Executor
// NEW: stored_genesis field — holds WASM b64 in-process so model never
//      needs to embed the raw string in context (fixes the loop).
// NEW: ReadFile, WasmExecuteLocal, WasmStore, WasmLoad actions wired.
// FIX: genesis_broker strips b64 from session_obs automatically.
use crate::agent_protocol::AgentAction;
use crate::distributed_scheduler::DistributedScheduler;
use crate::genesis_broker;
use crate::lazarus_vault::LazarusVault;
use crate::memory::MemoryPalace;
use crate::server::VmoState;
use crate::wasm_registry::WasmRegistry;
use crate::wasm_runtime::WasmRuntime;
use base64::{engine::general_purpose, Engine as _};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FsmState {
    Idle,
    Executing,
    Complete,
    Fault,
}

pub struct RoboticExecutor {
    pub current_state: FsmState,
    /// Stores the most recent GENESIS_PAYLOAD b64 in-process.
    /// The model never sees the raw string; genesis_broker provides an alias.
    pub stored_genesis: Option<String>,
    /// Name hint from the last synthesize_wasm call, for wasm_store default.
    pub last_func_hint: Option<String>,
}

impl RoboticExecutor {
    pub fn new() -> Self {
        Self { current_state: FsmState::Idle, stored_genesis: None, last_func_hint: None }
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
        dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>, String> {
        self.transition(FsmState::Executing);
        let mut raw_results: Vec<String> = Vec::new();

        for action in actions {
            if self.current_state == FsmState::Complete || self.current_state == FsmState::Fault {
                break;
            }

            match action {
                // ── Shell ─────────────────────────────────────────────
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
                        raw_results.push(stdout);
                    }
                    if !stderr.is_empty() {
                        raw_results.push(format!("STDERR: {}", stderr));
                    }
                }

                // ── File I/O ──────────────────────────────────────────
                AgentAction::ReadFile { path } => {
                    match crate::sandbox::validate_path(&path) {
                        Ok(safe) => match tokio::fs::read_to_string(&safe).await {
                            Ok(content) => {
                                // Truncate to 8KB to avoid context window bombs
                                let preview = if content.len() > 8192 {
                                    format!("{}\n... [TRUNCATED at 8192 chars]", &content[..8192])
                                } else {
                                    content
                                };
                                raw_results.push(format!("FILE_CONTENT[{}]:\n{}", path, preview));
                            }
                            Err(e) => raw_results.push(format!("READ_FAULT[{}]: {}", path, e)),
                        },
                        Err(e) => raw_results.push(format!("SANDBOX_VIOLATION: {}", e)),
                    }
                }

                AgentAction::WriteFile { path, content } => {
                    if path.starts_with("src/kernel") || path == "Cargo.toml" {
                        raw_results.push(format!("WRITE_BLOCKED: {} is DNA-protected", path));
                        continue;
                    }
                    match crate::sandbox::validate_path(&path) {
                        Ok(safe) => {
                            tokio::fs::write(&safe, &content).await.map_err(|e| e.to_string())?;
                            raw_results.push(format!("FILE_WRITTEN: {}", path));
                        }
                        Err(e) => raw_results.push(format!("SANDBOX_VIOLATION: {}", e)),
                    }
                }

                // ── Git ───────────────────────────────────────────────
                AgentAction::GitAction { command, message } => {
                    let msg = message.clone().unwrap_or_default();
                    let target = msg
                        .split_whitespace()
                        .last()
                        .unwrap_or("nexus-patch")
                        .trim_matches(|c| c == '\'' || c == '"' || c == '.');
                    let git_cmd = match command.as_str() {
                        "status" => "git status".to_string(),
                        "branch" => format!("git checkout -B {}", target),
                        "checkout" => format!("git checkout {}", target),
                        "commit" => format!("git add -A && git commit -m \"{}\"", msg),
                        "push" => "git push --set-upstream origin HEAD".to_string(),
                        "diff" => "git diff --staged".to_string(),
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
                    raw_results.push(format!("GIT[{}]: {}", command, stdout));
                    if !stderr.is_empty() {
                        raw_results.push(format!("GIT_STDERR: {}", stderr));
                    }
                }

                // ── Lazarus ───────────────────────────────────────────
                AgentAction::LazarusSnapshot { path, tag } => {
                    let tag_str = tag.as_deref().unwrap_or("auto");
                    match LazarusVault::auto_snapshot(&path, tag_str).await {
                        Ok(snap) => {
                            raw_results.push(format!("SNAPSHOT_OK: {} → {}", snap.path, snap.id))
                        }
                        Err(e) => raw_results.push(format!("SNAPSHOT_FAULT: {}", e)),
                    }
                }

                // ── WASM Forge ────────────────────────────────────────
                AgentAction::SynthesizeWasm { code } => {
                    let _ = LazarusVault::auto_snapshot("src/", "pre_forge").await;
                    match crate::wasm_forge::WasmForge::synthesize_cell(&code).await {
                        Ok(b64) => {
                            println!(">> [FORGE] Dispatching KERNEL_GENESIS payload to UI.");

                            // ROOT CAUSE FIX: store b64 in-process, NOT in session_obs
                            self.stored_genesis = Some(b64.clone());

                            // Infer func name from code for alias hint
                            let func_hint = code
                                .split("fn ")
                                .nth(1)
                                .and_then(|s| s.split('(').next())
                                .unwrap_or("execute_cell")
                                .to_string();
                            self.last_func_hint = Some(func_hint.clone());

                            // Send KERNEL_GENESIS to UI via WS
                            let _ws_payload = serde_json::json!({
                                "type": "KERNEL_GENESIS",
                                "payload": b64
                            })
                            .to_string();

                            raw_results.push("WASM_SYNTHESIS_COMPLETE".to_string());
                            // Push GENESIS_PAYLOAD for genesis_broker to intercept
                            raw_results.push(format!("GENESIS_PAYLOAD:{}", b64));
                        }
                        Err(e) => raw_results.push(format!("FORGE_ERROR: {}", e)),
                    }
                }

                // ── WasmExecuteLocal — uses stored b64, no raw string needed ──
                AgentAction::WasmExecuteLocal { func_name, input } => match &self.stored_genesis {
                    None => raw_results.push(
                        "WASM_EXEC_FAULT: No synthesized cell in memory. \
                             Run synthesize_wasm first."
                            .to_string(),
                    ),
                    Some(b64) => match general_purpose::STANDARD.decode(b64) {
                        Err(e) => raw_results.push(format!("B64_FAULT: {}", e)),
                        Ok(bytes) => match WasmRuntime::run_bytes(&bytes, &func_name, input) {
                            Ok(result) => {
                                raw_results.push(format!(
                                    "WASM_LOCAL_RESULT: {}({}) = {}",
                                    func_name, input, result
                                ));
                            }
                            Err(e) => raw_results.push(format!("WASM_EXEC_FAULT: {}", e)),
                        },
                    },
                },

                // ── WasmStore — persist named cell to registry ─────────
                AgentAction::WasmStore { name, func_name } => match &self.stored_genesis {
                    None => raw_results
                        .push("WASM_STORE_FAULT: No cell in memory to store.".to_string()),
                    Some(b64) => {
                        let source =
                            self.last_func_hint.clone().unwrap_or_else(|| func_name.clone());
                        match WasmRegistry::new(".nexus_wasm_registry.db").await {
                            Ok(reg) => match reg.store(&name, b64, &source).await {
                                Ok(_) => {
                                    raw_results.push(format!("WASM_STORED: {} → registry", name))
                                }
                                Err(e) => raw_results.push(format!("WASM_STORE_DB_FAULT: {}", e)),
                            },
                            Err(e) => raw_results.push(format!("WASM_REGISTRY_OPEN_FAULT: {}", e)),
                        }
                    }
                },

                // ── WasmLoad — load named cell and execute ─────────────
                AgentAction::WasmLoad { name, func_name, input } => {
                    match WasmRegistry::new(".nexus_wasm_registry.db").await {
                        Err(e) => raw_results.push(format!("WASM_REGISTRY_FAULT: {}", e)),
                        Ok(reg) => match reg.get(&name).await {
                            Err(e) => raw_results.push(format!("WASM_LOAD_FAULT: {}", e)),
                            Ok(None) => raw_results
                                .push(format!("WASM_NOT_FOUND: '{}' not in registry", name)),
                            Ok(Some(cell)) => {
                                match general_purpose::STANDARD.decode(&cell.wasm_b64) {
                                    Err(e) => raw_results.push(format!("B64_FAULT: {}", e)),
                                    Ok(bytes) => {
                                        match WasmRuntime::run_bytes(&bytes, &func_name, input) {
                                            Ok(result) => {
                                                // Also update stored_genesis for chaining
                                                self.stored_genesis = Some(cell.wasm_b64.clone());
                                                raw_results.push(format!(
                                                    "WASM_LOAD_RESULT: {}::{}({}) = {}",
                                                    name, func_name, input, result
                                                ));
                                            }
                                            Err(e) => {
                                                raw_results.push(format!("WASM_EXEC_FAULT: {}", e))
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }

                // ── Distributed WASM — alias-aware ────────────────────
                AgentAction::ExecuteDistributedWasm { wasm_b64, func_name, input } => {
                    // Resolve alias if model used WASM_CELL_READY shorthand
                    let resolved =
                        genesis_broker::resolve_b64(&wasm_b64, self.stored_genesis.as_deref());
                    match resolved {
                        None => raw_results.push(
                            "DIST_WASM_FAULT: No cell available. \
                             Run synthesize_wasm or wasm_load first."
                                .to_string(),
                        ),
                        Some(real_b64) => match general_purpose::STANDARD.decode(&real_b64) {
                            Err(e) => raw_results.push(format!("B64_FAULT: {}", e)),
                            Ok(bytes) => match dist.execute(bytes, &func_name, input).await {
                                Ok(res) => raw_results.push(format!(
                                    "CLUSTER_EXEC_SUCCESS: {}({}) = {}",
                                    func_name, input, res
                                )),
                                Err(e) => raw_results.push(format!("CLUSTER_EXEC_FAULT: {}", e)),
                            },
                        },
                    }
                }

                AgentAction::Halt => {
                    raw_results.push("MISSION_HALTED".to_string());
                    self.transition(FsmState::Complete);
                    break;
                }

                _ => raw_results.push("ACTION_PENDING".into()),
            }
        }

        // ROOT CAUSE FIX: strip GENESIS_PAYLOAD from results before returning
        // so session_obs never contains the raw base64 string.
        let (clean, b64_opt) = genesis_broker::extract(&raw_results);
        if let Some(b64) = b64_opt {
            self.stored_genesis = Some(b64);
        }

        Ok(clean)
    }
}
