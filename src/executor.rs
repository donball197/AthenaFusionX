// src/executor.rs - Extended with new actions (secure & compiling)
use anyhow::{Context, Result, anyhow};
use std::process::Command;
use std::fs;
use crate::agent_protocol::AgentAction;
use crate::sandbox;
use crate::component_runner::AgentHandle;
use crate::diff::DiffEngine;

const ALLOWED_BINARIES: &[&str] = &[
    "cargo", "rustc", "clippy-driver", "npm", "node", "git", "ls", "cat", "grep", 
    "find", "echo", "mkdir", "rm", "cp", "mv", "zig", "make"
];

pub struct Executor;

impl Executor {
    fn execute_command_safe(command: &str) -> Result<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() { return Err(anyhow!("Empty command")); }

        let binary = parts[0];
        if !ALLOWED_BINARIES.contains(&binary) {
            return Err(anyhow!("SECURITY_VIOLATION: Binary '{}' not allowed", binary));
        }

        let mut cmd = Command::new(binary);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }

        let output = cmd.output().context("Command execution failed")?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if !output.status.success() {
            Err(anyhow!("Command failed: {}", String::from_utf8_lossy(&output.stderr)))
        } else {
            Ok(stdout)
        }
    }

    pub async fn execute_actions(actions: &[AgentAction]) -> Result<Vec<String>> {
        let mut observations = Vec::new();

        for action in actions {
            match action {
                AgentAction::ReadFile { path } => {
                    let safe = sandbox::validate_path(path)?;
                    match fs::read_to_string(&safe) {
                        Ok(c) => observations.push(format!("FILE_READ_SUCCESS: {}\nCONTENT:\n{}", path, c)),
                        Err(e) => observations.push(format!("FILE_READ_ERROR: {} - {}", path, e)),
                    }
                }
                AgentAction::WriteFile { path, content } => {
                    let safe = sandbox::validate_path(path)?;
                    if let Some(p) = safe.parent() { let _ = fs::create_dir_all(p); }
                    fs::write(&safe, content)?;
                    observations.push(format!("WRITE_SUCCESS: {}", path));
                }
                AgentAction::PatchFile { path, search, replace } => {
                    let safe = sandbox::validate_path(path)?;
                    let orig = fs::read_to_string(&safe)?;
                    let new = DiffEngine::apply(&orig, search, replace)?;
                    fs::write(&safe, new)?;
                    observations.push(format!("PATCH_SUCCESS: {}", path));
                }
                AgentAction::ExecuteCommand { command } => {
                    match Self::execute_command_safe(command) {
                        Ok(out) => observations.push(format!("COMMAND_SUCCESS:\n{}", out)),
                        Err(e) => observations.push(format!("COMMAND_ERROR: {}", e)),
                    }
                }
                AgentAction::ListDirectory { path } => {
                    let safe = sandbox::validate_path(path)?;
                    let mut out = format!("DIRECTORY LISTING [{}]:\n", path);
                    if let Ok(entries) = fs::read_dir(&safe) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !name.starts_with('.') {
                                out.push_str(&format!("  {}\n", name));
                            }
                        }
                    }
                    observations.push(out);
                }
                AgentAction::DeleteFile { path } => {
                    let safe = sandbox::validate_path(path)?;
                    if safe.is_dir() { fs::remove_dir_all(&safe)?; } else { fs::remove_file(&safe)?; }
                    observations.push(format!("DELETE_SUCCESS: {}", path));
                }
                AgentAction::SearchCodebase { pattern } => {
                    let output = Command::new("grep").args(["-rn", pattern, "src", "ui/src"]).output()?;
                    observations.push(format!("SEARCH_RESULTS: {}\n{}", pattern, String::from_utf8_lossy(&output.stdout)));
                }
                
                // === NEW ACTIONS IMPLEMENTATION ===
                AgentAction::CreateDirectory { path } => {
                    let safe = sandbox::validate_path(path)?;
                    fs::create_dir_all(&safe)?;
                    observations.push(format!("DIRECTORY_CREATED: {}", path));
                }
                AgentAction::RunCargo { command } => {
                    let allowed = ["check", "build", "test", "clippy", "fmt", "audit", "flamegraph"];
                    if !allowed.contains(&command.as_str()) {
                        return Err(anyhow!("Cargo subcommand not allowed: {}", command));
                    }
                    // SUTURE: Passed reference to prevent borrow checker failure
                    let output = Command::new("cargo").arg(&command).output()?;
                    observations.push(format!("CARGO_{}:\n{}", command.to_uppercase(), String::from_utf8_lossy(&output.stdout)));
                }
                AgentAction::GitOperation { operation, args } => {
                    let allowed_ops = ["status", "log", "diff", "branch"];
                    if !allowed_ops.contains(&operation.as_str()) {
                        return Err(anyhow!("Git operation not allowed: {}", operation));
                    }
                    let mut cmd = Command::new("git");
                    cmd.arg(operation);
                    cmd.args(args);
                    let output = cmd.output()?;
                    observations.push(format!("GIT_{}: {}", operation.to_uppercase(), String::from_utf8_lossy(&output.stdout)));
                }
                AgentAction::SearchReplaceMulti { pattern, replace: _, file_pattern } => {
                    observations.push(format!("MULTI_REPLACE: pattern='{}' in files matching '{}'", pattern, file_pattern));
                    // TODO: Implement full multi-file version later
                }
                AgentAction::RunTests { test_filter } => {
                    let mut cmd = Command::new("cargo");
                    cmd.arg("test");
                    if let Some(filter) = test_filter {
                        cmd.arg(filter);
                    }
                    let output = cmd.output()?;
                    observations.push(format!("TEST_RUN:\n{}", String::from_utf8_lossy(&output.stdout)));
                }
                AgentAction::Message { message } => {
                    observations.push(format!("MESSAGE: {}", message));
                }
                AgentAction::GenerateReport { data } => {
                    observations.push(format!("REPORT: {}", serde_json::to_string_pretty(data)?));
                }
            }
        }
        Ok(observations)
    }

    pub async fn execute_handle(_handle: &AgentHandle, actions: Vec<AgentAction>) -> Result<Vec<String>> {
        Self::execute_actions(&actions).await
    }
}
