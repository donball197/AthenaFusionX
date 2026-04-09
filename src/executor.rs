// src/executor.rs - Hybrid Master Zenith Version
use anyhow::Result;
use std::process::Command;
use std::fs;
use crate::agent_protocol::AgentAction;
use crate::sandbox;
use crate::component_runner::AgentHandle;

pub struct Executor;

impl Executor {
    pub async fn execute_handle(_handle: &AgentHandle, actions: Vec<AgentAction>) -> Result<Vec<String>> {
        Self::execute_actions(&actions).await
    }

    pub async fn execute_actions(actions: &[AgentAction]) -> Result<Vec<String>> {
        let mut observations = Vec::new();
        for action in actions {
            match action {
                AgentAction::SynthesizeZig { path, logic } => {
                    let safe = sandbox::validate_path(path)?;
                    if let Some(p) = safe.parent() { fs::create_dir_all(p)?; }
                    fs::write(&safe, logic)?;
                    println!("\x1b[33m>> [ZIG] Synthesizing Reflex: {}...\x1b[0m", path);
                    let output = Command::new("zig")
                        .args(["build-exe", &safe.to_string_lossy(), "-O", "ReleaseSmall", &format!("-femit-bin={}.bin", path)])
                        .output()?;
                    if output.status.success() {
                        observations.push(format!("ZIG_SUCCESS: Binary seated at {}.bin", path));
                    } else {
                        observations.push(format!("ZIG_ERROR: {}", String::from_utf8_lossy(&output.stderr)));
                    }
                }
                AgentAction::ExecuteCommand { command } => {
                    let output = Command::new("sh").args(["-c", command]).output()?;
                    observations.push(format!("EXEC_SUCCESS: {}", String::from_utf8_lossy(&output.stdout)));
                }
                AgentAction::ReadFile { path } => {
                    let safe = sandbox::validate_path(path)?;
                    let _ = fs::read_to_string(&safe)?;
                    observations.push(format!("READ_SUCCESS: {}", path));
                }
                _ => observations.push("ACTION_PENDING_IMPLEMENTATION".to_string()),
            }
        }
        Ok(observations)
    }
}
