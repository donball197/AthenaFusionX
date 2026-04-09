// src/executor.rs - v3.3.1 SOVEREIGN SHIELD (CLEANSED)
use anyhow::{Result, anyhow};
use std::process::Command;
use std::fs;
use crate::agent_protocol::AgentAction;
use crate::sandbox;

const ALLOWED_BINARIES: &[&str] = &[
    "cargo", "git", "ls", "cat", "grep", "find", "echo", "mkdir", "rm", "zig", "make", "podman", "npm"
];

const PROTECTED_CORE: &[&str] = &["src/lib.rs", "src/main.rs"];
const OVERRIDE_KEY: &str = "OVERRIDE_SOVEREIGN_GUARD";

pub struct Executor;

impl Executor {
    pub async fn execute_handle(_handle: &crate::component_runner::AgentHandle, actions: Vec<AgentAction>) -> Result<Vec<String>> {
        Self::execute_actions(&actions).await
    }

    fn check_guard(path: &str, content: Option<&str>) -> Result<()> {
        if PROTECTED_CORE.contains(&path) {
            match content {
                Some(c) if c.contains(OVERRIDE_KEY) => {
                    eprintln!("\x1b[33m>> [GUARD] Override detected for core strand: {}\x1b[0m", path);
                    Ok(())
                },
                _ => Err(anyhow!("SECURITY_VIOLATION: Modification of core file '{}' is restricted. Use {} to authorize.", path, OVERRIDE_KEY))
            }
        } else {
            Ok(())
        }
    }

    pub async fn execute_actions(actions: &[AgentAction]) -> Result<Vec<String>> {
        let mut observations = Vec::new();

        for action in actions {
            match action {
                AgentAction::ReadFile { path } => {
                    let safe = sandbox::validate_path(path)?;
                    match fs::read_to_string(&safe) {
                        Ok(content) => observations.push(format!("READ_SUCCESS: {}\n{}", path, content)),
                        Err(e) => observations.push(format!("READ_ERROR: {} -> {}", path, e)),
                    }
                }
                AgentAction::WriteFile { path, content } => {
                    Self::check_guard(path, Some(content))?;
                    let safe = sandbox::validate_path(path)?;
                    if let Some(parent) = safe.parent() { let _ = fs::create_dir_all(parent); }
                    fs::write(&safe, content)?;
                    observations.push(format!("WRITE_SUCCESS: {}", path));
                }
                AgentAction::RunCargo { command } => {
                    let parts: Vec<&str> = command.split_whitespace().collect();
                    let allowed = ["check", "build", "test", "clippy", "run"];
                    if parts.is_empty() || !allowed.contains(&parts[0]) {
                        observations.push(format!("CARGO_ERROR: Command '{}' not allowed", parts.first().unwrap_or(&"")));
                        continue;
                    }
                    let output = Command::new("cargo").args(parts).output()?;
                    observations.push(format!("CARGO_SUCCESS:\n{}", String::from_utf8_lossy(&output.stdout)));
                }
                AgentAction::ExecuteCommand { command } => {
                    let parts: Vec<&str> = command.split_whitespace().collect();
                    if parts.is_empty() || !ALLOWED_BINARIES.contains(&parts[0]) {
                        observations.push(format!("SECURITY_VIOLATION: Binary restricted"));
                    } else {
                        let output = Command::new("sh").args(["-c", command]).output()?;
                        observations.push(format!("EXEC_SUCCESS: {}", String::from_utf8_lossy(&output.stdout)));
                    }
                }
                AgentAction::SpawnDrone { image, command, mount_path } => {
                    let _safe = sandbox::validate_path(&mount_path)?;
                    let output = Command::new("podman")
                        .args(["run", "--rm", "--network", "none", "-v", &format!("{}:/workspace:z", mount_path), "-w", "/workspace", image, "sh", "-c", command])
                        .output()?;
                    observations.push(format!("DRONE_EXIT: {}\nSTDOUT: {}", output.status, String::from_utf8_lossy(&output.stdout)));
                }
                AgentAction::Message { message } => { observations.push(format!("MSG: {}", message)); }
                _ => observations.push("ACTION_PENDING_IMPLEMENTATION".into()),
            }
        }
        Ok(observations)
    }
}
