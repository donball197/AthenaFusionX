use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::sync::broadcast::Sender;
use std::process::Command;
use std::fs;
use std::path::Path;
use crate::agent_protocol::AgentAction;
use crate::component_runner::AgentHandle;
use crate::diff::DiffEngine; // SUTURE: Import the surgical unit

pub struct Executor;

impl Executor {
    fn check_whitelist(cmd: &str) -> bool {
        let whitelist = ["cargo", "rustc", "ls", "cat", "rm", "grep", "find", "echo"];
        let primary_bin = cmd.split_whitespace().next().unwrap_or("");
        whitelist.contains(&primary_bin)
    }

    pub async fn execute_handle(_handle: &AgentHandle, actions: Vec<AgentAction>) -> Result<Vec<String>> {
        let mut observations = Vec::new();
        for action in actions {
            match action {
                AgentAction::ReadFile { path } => {
                    match fs::read_to_string(&path) {
                        Ok(c) => observations.push(format!("FILE_READ_SUCCESS: {}\nCONTENT:\n{}", path, c)),
                        Err(e) => observations.push(format!("FILE_READ_ERROR: {} - {}", path, e)),
                    }
                }
                AgentAction::WriteFile { path, content } => {
                    if let Some(parent) = Path::new(&path).parent() { let _ = fs::create_dir_all(parent); }
                    fs::write(&path, content).context("FS_WRITE_FAIL")?;
                    observations.push(format!("FILE_WRITE_SUCCESS: {}", path));
                }
                AgentAction::ExecuteCommand { command } => {
                    if Self::check_whitelist(&command) {
                        let output = Command::new("sh").args(["-c", &command]).output()?;
                        observations.push(format!("COMMAND_SUCCESS: {}\nSTDOUT: {}", command, String::from_utf8_lossy(&output.stdout)));
                    } else {
                        observations.push(format!("SECURITY_VIOLATION: Binary unauthorized."));
                    }
                }
                _ => observations.push("ACTION_PENDING".into()),
            }
        }
        Ok(observations)
    }

    pub async fn execute_swarm_json(json_text: &str, tx: &Sender<String>) -> Result<Vec<String>> {
        let mut observations = Vec::new();
        let start = json_text.find('[').context("JSON_FRACTURE")?;
        let end = json_text.rfind(']').context("JSON_FRACTURE")?;
        let actions: Vec<Value> = serde_json::from_str(&json_text[start..=end])?;
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

        for action in actions {
            let op = action.get("action").or(action.get("op")).or(action.get("opcode")).and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            
            let get_param = |key: &str| {
                action.get(key).or_else(|| action.get("args").and_then(|a| a.get(key))).or_else(|| action.get("params").and_then(|p| p.get(key))).and_then(|v| v.as_str())
            };

            let target = get_param("target").or(get_param("path")).or(get_param("file")).or(get_param("selector")).unwrap_or("body");

            // --- SURGICAL PATCHING ---
            if op == "patch_file" || op == "patch" {
                let search = get_param("search").unwrap_or("");
                let replace = get_param("replace").unwrap_or("");
                
                // LAZARUS FAILSAFE
                let backup_dir = format!(".lazarus_vault/{}", timestamp);
                let _ = fs::create_dir_all(&backup_dir);
                let _ = Command::new("cp").args(["-r", "src", &backup_dir]).output();

                let original = fs::read_to_string(target).context("READ_FAIL_BEFORE_PATCH")?;
                match DiffEngine::apply(&original, search, replace) {
                    Ok(new_content) => {
                        fs::write(target, new_content).context("PATCH_WRITE_FAIL")?;
                        observations.push(format!("PATCH_SUCCESS: {}", target));
                        let _ = tx.send(format!(">> [SURGERY] Applied patch to {}", target));
                    },
                    Err(e) => observations.push(format!("PATCH_ERROR: {} - {}", target, e)),
                }
                continue;
            }

            // --- VISION ---
            if op == "capture_visor" || op == "screenshot" {
                let _ = tx.send(json!({"type": "capture_request", "payload": target}).to_string());
                observations.push(format!("VISOR_SIGNAL_SENT: target='{}'. Mission Accomplished.", target));
                continue;
            }

            // --- FS WRITES ---
            if op == "write_file" || op == "create_file" {
                let content = get_param("content").or(get_param("code")).unwrap_or("");
                let backup_dir = format!(".lazarus_vault/{}", timestamp);
                let _ = fs::create_dir_all(&backup_dir);
                let _ = Command::new("cp").args(["-r", "src", &backup_dir]).output();
                if let Some(parent) = Path::new(target).parent() { let _ = fs::create_dir_all(parent); }
                fs::write(target, content).context("FS_WRITE_FAIL")?;
                observations.push(format!("FILE_WRITE_SUCCESS: {}", target));
            } 
            // --- FS READS ---
            else if op == "read_file" || op == "read" {
                match fs::read_to_string(target) {
                    Ok(c) => observations.push(format!("FILE_READ_SUCCESS: {}\nCONTENT:\n{}", target, c)),
                    Err(e) => observations.push(format!("FILE_READ_ERROR: {} - {}", target, e)),
                }
            }
            // --- COMMAND EXEC ---
            else if op.contains("command") || op == "run" {
                let cmd_val = get_param("command").or(get_param("cmd")).unwrap_or("");
                if !cmd_val.is_empty() {
                    if Self::check_whitelist(cmd_val) {
                        let output = Command::new("sh").args(["-c", cmd_val]).output()?;
                        observations.push(format!("COMMAND_SUCCESS: {}\nSTDOUT: {}", cmd_val, String::from_utf8_lossy(&output.stdout)));
                    } else {
                        observations.push(format!("SECURITY_VIOLATION: Binary unauthorized in '{}'", cmd_val));
                    }
                }
            }
        }
        Ok(observations)
    }
}
