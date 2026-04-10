/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/executor.rs - v4.1.0 (Scraper Muscle Integration)
use anyhow::{Result, anyhow};
use std::process::Command;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::agent_protocol::AgentAction;
use crate::{sandbox, scraper_drone};
use crate::memory::MemoryPalace;

const ALLOWED_BINARIES: &[&str] = &[
    "cargo", "git", "ls", "cat", "grep", "find", "echo", "mkdir", "rm", "zig", "make", "podman", "npm"
];

const PROTECTED_CORE: &[&str] = &["src/lib.rs", "src/main.rs"];
const OVERRIDE_KEY: &str = "OVERRIDE_SOVEREIGN_GUARD";

pub struct Executor;

impl Executor {
    fn check_guard(path: &str, content: Option<&str>) -> Result<()> {
        if PROTECTED_CORE.contains(&path) {
            match content {
                Some(c) if c.contains(OVERRIDE_KEY) => {
                    eprintln!("\x1b[33m>> [GUARD] Override detected for core strand: {}\x1b[0m", path);
                    Ok(())
                },
                _ => Err(anyhow!("SECURITY_VIOLATION: Modification of core file '{}' is restricted.", path))
            }
        } else {
            Ok(())
        }
    }

    pub async fn execute_handle(
        _handle: &crate::component_runner::AgentHandle,
        actions: Vec<AgentAction>,
        memory: Arc<Mutex<MemoryPalace>>,
    ) -> Result<Vec<String>> {
        let mut observations = Vec::new();

        for action in actions {
            match action {
                AgentAction::ScrapeTarget { url } => {
                    println!("\x1b[32m>> [MOTOR] Deploying Scraper Drone to {}...\x1b[0m", url);
                    match scraper_drone::execute_mission(&url, memory.clone()).await {
                        Ok(_) => observations.push(format!("SCRAPE_SUCCESS: {} imprinted to memory.", url)),
                        Err(e) => observations.push(format!("SCRAPE_ERROR: {}", e)),
                    }
                }
                AgentAction::QueryMemory { query } => {
                    let palace = memory.lock().await;
                    match palace.search_memories(&query).await {
                        Ok(results) => observations.push(format!("MEMORY_QUERY: Found {} strands for '{}'.", results.len(), query)),
                        Err(e) => observations.push(format!("MEMORY_ERROR: {}", e)),
                    }
                }
                AgentAction::ReadFile { path } => {
                    let safe = sandbox::validate_path(&path)?;
                    match fs::read_to_string(&safe) {
                        Ok(content) => observations.push(format!("READ_SUCCESS: {}\n{}", path, content)),
                        Err(e) => observations.push(format!("READ_ERROR: {}", e)),
                    }
                }
                AgentAction::WriteFile { path, content } => {
                    Self::check_guard(&path, Some(&content))?;
                    let safe = sandbox::validate_path(&path)?;
                    if let Some(parent) = safe.parent() { let _ = fs::create_dir_all(parent); }
                    fs::write(&safe, &content)?;
                    observations.push(format!("WRITE_SUCCESS: {}", path));
                }
                AgentAction::ExecuteCommand { command } => {
                    let parts: Vec<&str> = command.split_whitespace().collect();
                    if parts.is_empty() || !ALLOWED_BINARIES.contains(&parts[0]) {
                        observations.push(format!("SECURITY_VIOLATION: Binary '{}' restricted.", parts.get(0).unwrap_or(&"")));
                    } else {
                        let output = Command::new("sh").args(["-c", &command]).output()?;
                        observations.push(format!("EXEC_SUCCESS: {}", String::from_utf8_lossy(&output.stdout)));
                    }
                }
                AgentAction::Halt => {
                    observations.push("SIGNAL: HALT_RECEIVED".to_string());
                }
                _ => observations.push("ACTION_PENDING_IMPLEMENTATION".into()),
            }
        }
        Ok(observations)
    }
}
