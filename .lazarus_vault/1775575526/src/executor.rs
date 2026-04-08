use crate::nexus_error::NexusError;
use serde_json::Value;
use tokio::sync::broadcast::Sender;
use std::process::Command;
use std::fs;
use std::path::Path;
use crate::agent_protocol::AgentAction;
use crate::component_runner::AgentHandle;

pub struct Executor;

impl Executor {
    pub async fn execute_handle(
        _handle: &AgentHandle,
        _actions: Vec<AgentAction>,
    ) -> Result<Vec<String>, NexusError> {
        Ok(vec![">> [AGENT]: Directive logged via component handle.".into()])
    }

    pub async fn execute_swarm_json(json_text: &str, tx: &Sender<String>) -> Result<(), NexusError> {
        if let Ok(actions) = serde_json::from_str::<Vec<Value>>(json_text) {
            println!("\x1b[32m>> [EXECUTOR] Action Matrix Engaged.\x1b[0m");
            let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

            for action in actions {
                // --- LIQUID LEXICON MAPPING ---
                let op = action.get("action").or_else(|| action.get("op")).and_then(|a| a.as_str()).unwrap_or("").to_lowercase();
                let target = action.get("target").or_else(|| action.get("file")).or_else(|| action.get("path")).and_then(|a| a.as_str()).unwrap_or("GLOBAL");
                
                // --- UNIFIED FS PROTOCOL (create/write/patch) ---
                if op.contains("file") || op == "patch" || op == "modify" {
                    // LAZARUS PROTOCOL: Full Source Backup before mutation
                    let backup_dir = format!(".lazarus_vault/{}", timestamp);
                    let _ = fs::create_dir_all(&backup_dir);
                    let _ = Command::new("cp").args(["-r", "src", &backup_dir]).output();
                    
                    let content = action.get("content").or_else(|| action.get("code")).and_then(|c| c.as_str()).unwrap_or("");
                    if !content.is_empty() {
                        if let Some(parent) = Path::new(target).parent() { let _ = fs::create_dir_all(parent); }
                        let _ = fs::write(target, content);
                        println!("\x1b[32m>> [LAZARUS] Snapshot {} secured. [FS] Updated: {}\x1b[0m", timestamp, target);
                        let _ = tx.send(format!(">> [LAZARUS] Snapshot saved. FS Updated: {}", target));
                    }
                }

                // --- UNIFIED EXEC PROTOCOL (command/shell/bash) ---
                let cmd = action.get("command").or_else(|| action.get("cmd")).or_else(|| action.get("args")).and_then(|c| c.as_str()).unwrap_or("");
                if !cmd.is_empty() || op.contains("shell") || op.contains("command") {
                    println!("\x1b[33m>> [EXEC] {}\x1b[0m", cmd);
                    let output = Command::new("sh").args(["-c", cmd]).output();
                    if let Ok(out) = output {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        if !stdout.is_empty() { println!("\x1b[38;5;244m{}\x1b[0m", stdout.trim()); }
                    }
                }
            }
        }
        Ok(())
    }
}
