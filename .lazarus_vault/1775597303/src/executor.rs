use anyhow::{Context, Result};
use serde_json::Value;
use tokio::sync::broadcast::Sender;
use std::process::Command;
use std::fs;
use std::path::Path;
use crate::agent_protocol::AgentAction;
use crate::component_runner::AgentHandle;

pub struct Executor;

impl Executor {
    pub async fn execute_handle(_handle: &AgentHandle, _actions: Vec<AgentAction>) -> Result<Vec<String>> {
        Ok(vec![">> [AGENT]: Action logged via actor handle.".into()])
    }

    pub async fn execute_swarm_json(json_text: &str, tx: &Sender<String>) -> Result<Vec<String>> {
        let mut observations = Vec::new();
        let start = json_text.find('[').context("JSON_FRACTURE: Missing start bracket")?;
        let end = json_text.rfind(']').context("JSON_FRACTURE: Missing end bracket")?;
        
        let actions: Vec<Value> = serde_json::from_str(&json_text[start..=end]).context("JSON_PARSE_FAIL")?;

        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

        // SOVEREIGN COMMAND WHITELIST
        let whitelist = ["cargo", "rustc", "ls", "cat", "rm", "grep", "find", "echo"];

        for action in actions {
            let op = action.get("action").or_else(|| action.get("op")).and_then(|a| a.as_str()).unwrap_or("").to_lowercase();
            let target = action.get("target").or_else(|| action.get("file")).or_else(|| action.get("path")).and_then(|a| a.as_str()).unwrap_or("GLOBAL");
            
            // --- FS WRITES & LAZARUS FAILSAFE ---
            if op == "write_file" || op == "patch" || op == "create_file" {
                let backup_dir = format!(".lazarus_vault/{}", timestamp);
                let _ = fs::create_dir_all(&backup_dir);
                let _ = Command::new("cp").args(["-r", "src", &backup_dir]).output();

                let content = action.get("content").or_else(|| action.get("code")).and_then(|c| c.as_str()).unwrap_or("");
                if !content.is_empty() {
                    if let Some(parent) = Path::new(target).parent() { let _ = fs::create_dir_all(parent); }
                    fs::write(target, content).context("FS_WRITE_FAIL")?;
                    observations.push(format!("FILE_WRITE_SUCCESS: {}", target));
                    let _ = tx.send(format!(">> [FS] Updated: {}", target));
                }
            } 
            // --- FS READS ---
            else if op == "read_file" || op == "read" {
                match fs::read_to_string(target) {
                    Ok(c) => observations.push(format!("FILE_READ_SUCCESS: {}\nCONTENT:\n{}", target, c)),
                    Err(e) => observations.push(format!("FILE_READ_ERROR: {} - {}", target, e)),
                }
            }
            // --- HARDENED COMMAND EXECUTION ---
            else if op.contains("command") || op == "run" || op == "execute_command" {
                let cmd = action.get("command").or_else(|| action.get("cmd")).or_else(|| action.get("args")).and_then(|c| c.as_str()).unwrap_or("");
                if !cmd.is_empty() {
                    // COMMAND GATE: Identify the binary
                    let primary_bin = cmd.split_whitespace().next().unwrap_or("");
                    
                    if whitelist.contains(&primary_bin) {
                        println!("\x1b[33m>> [EXEC] {}\x1b[0m", cmd);
                        let output = Command::new("sh").args(["-c", cmd]).output()?;
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        
                        if output.status.success() {
                            observations.push(format!("COMMAND_SUCCESS: {}\nSTDOUT: {}", cmd, stdout));
                        } else {
                            observations.push(format!("COMMAND_ERROR: {}\nSTDERR: {}", cmd, stderr));
                        }
                    } else {
                        // SECURITY BLOCK
                        let violation = format!("SECURITY_VIOLATION: Command '{}' uses unauthorized binary '{}'.", cmd, primary_bin);
                        println!("\x1b[31m!! {}\x1b[0m", violation);
                        let _ = tx.send(format!("!! {}", violation));
                        observations.push(violation);
                    }
                }
            }
        }
        Ok(observations)
    }
}
