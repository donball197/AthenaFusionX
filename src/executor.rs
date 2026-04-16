use crate::agent_protocol::AgentAction;
use crate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::process::Command;
use tokio::fs;

pub struct Executor;

impl Executor {
    pub async fn execute_handle(actions: Vec<AgentAction>, _memory: Arc<Mutex<MemoryPalace>>) -> Result<Vec<String>, String> {
        let mut results = Vec::new();

        for action in actions {
            match action {
                AgentAction::ExecuteCommand { command } => {
                    let output = Command::new("sh")
                        .arg("-c")
                        .arg(&command)
                        .output()
                        .await
                        .map_err(|e| format!("Execution failed: {}", e))?;

                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    
                    let mut res = String::new();
                    if !stdout.is_empty() { res.push_str(&stdout); }
                    if !stderr.is_empty() { 
                        if !res.is_empty() { res.push('\n'); }
                        res.push_str(&format!("STDERR: {}", stderr)); 
                    }
                    if res.is_empty() { res.push_str("Command executed silently."); }
                    
                    results.push(res);
                },
                AgentAction::WriteFile { path, content } => {
                    fs::write(&path, &content).await.map_err(|e| e.to_string())?;
                    results.push(format!("File '{}' written successfully.", path));
                },
                AgentAction::SystemStatus { data } => {
                    // Logs or processes the JSON telemetry payload from the Ghost
                    results.push(format!("SYSTEM_STATUS_LOGGED: {}", data));
                },
                AgentAction::Halt => {
                    results.push("HALT_SIGNAL".to_string());
                }
            }
        }
        Ok(results)
    }
}
