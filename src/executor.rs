// src/executor.rs — v3.1 Rigid FSM Robotic Executor
// Fully integrated with VMO, Memory, and System Actions.

use crate::agent_protocol::AgentAction;
use crate::memory::MemoryPalace;
use crate::server::VmoState;
use crate::lazarus_vault::LazarusVault;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::process::Command;
use tokio::fs;
use std::path::Path;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FsmState {
    Idle,
    Executing,
    Complete,
    Fault,
}

pub struct RoboticExecutor {
    pub current_state: FsmState,
}

impl RoboticExecutor {
    pub fn new() -> Self {
        Self {
            current_state: FsmState::Idle,
        }
    }

    /// transition: Only moves forward, never backward.
    pub fn transition(&mut self, next_state: FsmState) {
        if self.current_state == FsmState::Complete || self.current_state == FsmState::Fault {
            return;
        }
        self.current_state = next_state;
    }

    pub async fn execute_handle(
        &mut self,
        actions: Vec<AgentAction>,
        memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
    ) -> Result<Vec<String>, String> {
        self.transition(FsmState::Executing);
        let mut results = Vec::new();

        for action in actions {
            // Abort if FSM reached a terminal state mid-array.
            if self.current_state == FsmState::Complete || self.current_state == FsmState::Fault {
                results.push("FSM_INTERRUPT: Terminal state reached. Halting execution.".to_string());
                break;
            }

            match action {
                AgentAction::ScrapeUrl { url } => {
                    match crate::scraper_drone::execute_mission(&url, memory.clone()).await {
                        Ok(res) => results.push(res),
                        Err(e) => results.push(format!("SCRAPE_FAULT: {}", e)),
                    }
                }

                AgentAction::GetVmoState => {
                    let state = vmo_state.lock().await;
                    let parent_str = state.last_parent.map(|h| format!("0x{:x}", h)).unwrap_or_else(|| "None".to_string());
                    let child_str = state.last_child.map(|h| format!("0x{:x}", h)).unwrap_or_else(|| "None".to_string());
                    results.push(format!("VMO_STATE: last_parent={}, last_child={}", parent_str, child_str));
                }

                AgentAction::VmoCreate => {
                    let handle = unsafe { crate::kernel::zx_vmo_create_native(4096) };
                    crate::kernel::register_handle(handle, crate::kernel::HandleType::Vmo(handle));
                    vmo_state.lock().await.last_parent = Some(handle);
                    results.push(format!("VMO_CREATE: 0x{:x}", handle));
                }

                AgentAction::VmoClone { parent_handle } => {
                    let parent_id = parse_handle(&parent_handle);
                    if parent_id == 0 {
                        results.push(format!("VMO_CLONE_ERROR: Invalid handle '{}'", parent_handle));
                    } else {
                        let child = unsafe { crate::kernel::ffi::zx_vmo_clone_native(parent_id, 0) };
                        if child == 0 {
                            results.push("VMO_CLONE_ERROR: Kernel rejected (OOM or invalid)".into());
                        } else {
                            crate::kernel::register_handle(child, crate::kernel::HandleType::Vmo(child));
                            let mut state = vmo_state.lock().await;
                            state.last_parent = Some(parent_id);
                            state.last_child = Some(child);
                            results.push(format!("VMO_CLONE_SUCCESS: Parent 0x{:x} -> Child 0x{:x} (COW Active)", parent_id, child));
                        }
                    }
                }

                AgentAction::VmoWrite { handle, data } => {
                    let handle_id = parse_handle(&handle);
                    if handle_id == 0 {
                        results.push(format!("VMO_WRITE_ERROR: Invalid handle '{}'", handle));
                    } else {
                        let write_bytes: Vec<u8> = if data.len() % 2 == 0 && data.len() >= 2 && data.chars().all(|c| c.is_ascii_hexdigit()) {
                            (0..data.len()).step_by(2).map(|i| u8::from_str_radix(&data[i..i + 2], 16).unwrap_or(0)).collect()
                        } else {
                            data.as_bytes().to_vec()
                        };
                        let rc = unsafe { crate::kernel::ffi::zx_vmo_write_native(handle_id, 0, write_bytes.as_ptr(), write_bytes.len()) };
                        if rc == 0 {
                            results.push(format!("VMO_WRITE_SUCCESS: {} bytes → 0x{:x} | '{}'", write_bytes.len(), handle_id, String::from_utf8_lossy(&write_bytes)));
                        } else {
                            results.push(format!("VMO_WRITE_ERROR: Kernel fault on 0x{:x}", handle_id));
                        }
                    }
                }

                AgentAction::VmoRead { handle } => {
                    let handle_id = parse_handle(&handle);
                    if handle_id == 0 {
                        results.push(format!("VMO_READ_ERROR: Invalid handle '{}'", handle));
                    } else {
                        let mut buffer = vec![0u8; 256];
                        let rc = unsafe { crate::kernel::ffi::zx_vmo_read_native(handle_id, 0, buffer.as_mut_ptr(), buffer.len()) };
                        if rc == 0 {
                            let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
                            let trimmed = &buffer[..end];
                            let ascii = String::from_utf8_lossy(trimmed).to_string();
                            let hex: String = trimmed.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
                            results.push(format!("VMO_READ_SUCCESS: 0x{:x}\n  ASCII: '{}'\n  HEX  : {}", handle_id, ascii, hex));
                        } else {
                            results.push(format!("VMO_READ_ERROR: Kernel fault on 0x{:x}", handle_id));
                        }
                    }
                }

                AgentAction::ExecuteCommand { command } => {
                    let blocked = ["rm -rf /", "mkfs", "dd if=/dev/random of=/dev/sd", ":(){ :|:& };:", "/etc/shadow", "chmod 777 /", "curl | bash", "wget | bash", "sudo rm -rf"];
                    if blocked.iter().any(|&b| command.contains(b)) {
                        results.push(format!("FIREWALL_BLOCK: Dangerous pattern in '{}'", command));
                        continue;
                    }

                    let output = Command::new("sh").arg("-c").arg(&command).output().await.map_err(|e| format!("Execution failed: {}", e))?;
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                    if !stdout.is_empty() && !stderr.is_empty() { results.push(format!("{}\n{}", stdout, stderr)); } 
                    else if !stdout.is_empty() { results.push(stdout); } 
                    else if !stderr.is_empty() { results.push(stderr); } 
                    else { results.push(format!("OK (exit: {})", output.status.code().unwrap_or(-1))); }
                }

                AgentAction::SearchMemory { query } => {
                    let palace = memory.lock().await;
                    let search_result = if query.to_uppercase().contains("RANDOM") { palace.fetch_random_strands(10).await } else { palace.search_memories(&query).await };
                    match search_result {
                        Ok(strands) => {
                            let mut out = format!("MEMORY_SEARCH for '{}':\n", query);
                            if strands.is_empty() { out.push_str("No strands found."); }
                            for s in strands { out.push_str(&format!("[{}] Dir: {} | Res: {}\n", s.timestamp, s.directive, s.result)); }
                            results.push(out);
                        }
                        Err(e) => results.push(format!("SEARCH_ERROR: {}", e)),
                    }
                }

                AgentAction::WriteFile { path, content } => {
                    let blocked = ["src/kernel", "Cargo.toml", "/etc/", "/bin/", "/usr/"];
                    if blocked.iter().any(|&p| path.contains(p)) {
                        results.push(format!("DNA_BLOCK: Write forbidden to '{}'", path));
                        continue;
                    }
                    if let Some(parent) = Path::new(&path).parent() {
                        if !parent.as_os_str().is_empty() { fs::create_dir_all(parent).await.map_err(|e| e.to_string())?; }
                    }
                    fs::write(&path, &content).await.map_err(|e| e.to_string())?;
                    results.push(format!("FILE_WRITTEN: '{}'", path));
                }

                AgentAction::ExportReport { title, content } => {
                    fs::create_dir_all("exports").await.map_err(|e| e.to_string())?;
                    let safe_title: String = title.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
                    
                    let final_title = if safe_title.trim_matches('_').is_empty() {
                        format!("report_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
                    } else { safe_title };

                    let filename = format!("exports/AUDIT_{}.md", final_title);

                    if fs::write(&filename, content).await.is_ok() {
                        results.push(format!("EXPORT_SUCCESS: {}", filename));
                        self.transition(FsmState::Complete); 
                    } else {
                        results.push(format!("EXPORT_FAULT: Failed to write {}", filename));
                    }
                }

                AgentAction::LazarusSnapshot { path, tag } => {
                    let tag_str = tag.unwrap_or_else(|| "ghost_auto".to_string());
                    match LazarusVault::auto_snapshot(&path, &tag_str).await {
                        Ok(snap) => results.push(format!("LAZARUS_SNAPSHOT: {} | Hash: {} | Tag: {}", path, &snap.hash[..12], tag_str)),
                        Err(e) => { results.push(format!("LAZARUS_FAULT: {}", e)); }
                    }
                }

                AgentAction::Halt => {
                    results.push("SEQUENCE_HALTED".to_string());
                    self.transition(FsmState::Complete);
                    break;
                }
            }
        }

        if self.current_state == FsmState::Executing {
            self.transition(FsmState::Complete);
        }

        Ok(results)
    }
}

fn parse_handle(s: &str) -> u32 {
    u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap_or(0)
}
