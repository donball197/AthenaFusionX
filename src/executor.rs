use crate::nexus_error::NexusError;
use crate::agent_protocol::AgentAction;
use crate::component_runner::{AgentHandle, Capabilities};
use crate::lazarus_vault::LazarusVault;
use crate::podman::PodmanShield;
use std::fs;

pub struct Executor;

impl Executor {
    pub async fn execute_handle(handle: &AgentHandle, actions: Vec<AgentAction>) -> Result<Vec<String>, NexusError> {
        let mut results = Vec::new();
        
        for action in actions {
            match action {
                AgentAction::ReadFile { path } => {
                    if !handle.capabilities.contains(Capabilities::READ_FS) {
                        results.push(format!("!! DENIED: ReadFile on {} (Lacks Capability)", path));
                        continue;
                    }
                    match fs::read_to_string(&path) {
                        Ok(content) => results.push(format!(">> [READ] {}:\n{}", path, content)),
                        Err(e) => results.push(format!("!! [READ_FAULT] {}: {}", path, e)),
                    }
                }
                AgentAction::WriteFile { path, content } => {
                    if !handle.capabilities.contains(Capabilities::WRITE_FS) {
                        results.push(format!("!! DENIED: WriteFile (Lacks Capability)"));
                        continue;
                    }
                    if std::path::Path::new(&path).exists() {
                        let _ = LazarusVault::auto_snapshot(&path, "Pre-AI Overwrite");
                    }
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    match fs::write(&path, &content) {
                        Ok(_) => results.push(format!(">> [WROTE] {} (Lazarus Snapshot Created)", path)),
                        Err(e) => results.push(format!("!! [WRITE_FAULT] {}: {}", path, e)),
                    }
                }
                AgentAction::ExecuteCommand { command } => {
                    if !handle.capabilities.contains(Capabilities::SYSTEM_EXEC) {
                        results.push(format!("!! DENIED: ExecuteCommand (Lacks Capability)"));
                        continue;
                    }
                    
                    let workspace = std::env::current_dir().unwrap().to_string_lossy().to_string();
                    match PodmanShield::execute_isolated(&command, &workspace).await {
                        Ok(val) => results.push(format!(">> [SHIELDED_EXEC]: {}", val)),
                        Err(e) => results.push(format!("!! [BIO-DOME_FAULT]: {}", e)),
                    }
                }
                _ => results.push("!! [AGENT]: Action rejected by protocol.".to_string()),
            }
        }
        Ok(results)
    }
}
