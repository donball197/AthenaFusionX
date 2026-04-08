use anyhow::Result;
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
    ) -> Result<Vec<String>> {
        Ok(vec![">> [AGENT]: Action logged via component handle.".into()])
    }

    pub async fn execute_swarm_json(json_text: &str, tx: &Sender<String>) -> Result<Vec<String>> {
        let mut observations = Vec::new();
        let start = json_text.find('[').unwrap_or(0);
        let end = json_text.rfind(']').unwrap_or(json_text.len().saturating_sub(1));
        let sanitized_json = if start <= end { &json_text[start..=end] } else { json_text };

        if let Ok(actions) = serde_json::from_str::<Vec<Value>>(sanitized_json) {
            let _ = tx.send(">> [EXECUTOR] Action Matrix Engaged.".to_string());
            println!("\x1b[32m>> [EXECUTOR] Action Matrix Engaged.\x1b[0m");
            let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

            for action in actions {
                let op = action.get("action").or_else(|| action.get("op")).and_then(|a| a.as_str()).unwrap_or("").to_lowercase();
                let target = action.get("target").or_else(|| action.get("file")).or_else(|| action.get("path")).and_then(|a| a.as_str()).unwrap_or("GLOBAL");
                
                if op == "read_file" || op == "read" {
                    match fs::read_to_string(target) {
                        Ok(content) => {
                            observations.push(format!("FILE_READ_SUCCESS: {}\nCONTENT:\n{}", target, content));
                            let _ = tx.send(format!(">> [FS] Read success: {}", target));
                        },
                        Err(e) => observations.push(format!("FILE_READ_ERROR: {} - {}", target, e)),
                    }
                }
                else if op.contains("file") || op == "patch" || op == "modify" {
                    let backup_dir = format!(".lazarus_vault/{}", timestamp);
                    let _ = fs::create_dir_all(&backup_dir);
                    let _ = Command::new("cp").args(["-r", "src", &backup_dir]).output();
                    
                    let content = action.get("content").or_else(|| action.get("code")).and_then(|c| c.as_str()).unwrap_or("");
                    if !content.is_empty() {
                        if let Some(parent) = Path::new(target).parent() { let _ = fs::create_dir_all(parent); }
                        match fs::write(target, content) {
                            Ok(_) => {
                                observations.push(format!("FILE_WRITE_SUCCESS: {}", target));
                                println!("\x1b[32m>> [LAZARUS] Snapshot {} secured. [FS] Updated: {}\x1b[0m", timestamp, target);
                                let _ = tx.send(format!(">> [LAZARUS] Snapshot secured. FS Updated: {}", target));
                            },
                            Err(e) => observations.push(format!("FILE_WRITE_ERROR: {} - {}", target, e)),
                        }
                    }
                }
                else if op.contains("command") || op.contains("shell") || op == "run" {
                    let cmd = action.get("command").or_else(|| action.get("cmd")).or_else(|| action.get("args")).and_then(|c| c.as_str()).unwrap_or("");
                    if !cmd.is_empty() {
                        println!("\x1b[33m>> [EXEC] {}\x1b[0m", cmd);
                        let output = Command::new("sh").args(["-c", cmd]).output();
                        match output {
                            Ok(out) => {
                                let stdout = String::from_utf8_lossy(&out.stdout);
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                observations.push(format!("COMMAND_RESULT:\nSTDOUT: {}\nSTDERR: {}", stdout, stderr));
                                if !stdout.is_empty() { println!("\x1b[38;5;244m{}\x1b[0m", stdout.trim()); }
                                let _ = tx.send(format!(">> [EXEC OUTPUT]: {}", stdout.trim()));
                            },
                            Err(e) => observations.push(format!("COMMAND_FAULT: {}", e)),
                        }
                    }
                }
            }
        }
        Ok(observations)
    }
}
