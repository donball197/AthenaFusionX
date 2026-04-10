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


// src/agent_protocol.rs - v3.4.0 (Halt variant added)
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    PatchFile { path: String, search: String, replace: String },
    ExecuteCommand { command: String },
    ListDirectory { path: String },
    DeleteFile { path: String },
    CreateDirectory { path: String },
    SearchCodebase { pattern: String },
    RunCargo { command: String },
    SpawnDrone { image: String, command: String, mount_path: String },
    SearchReplaceMulti { pattern: String, replace: String, file_pattern: String },
    Message { message: String },
    Stasis { duration_ms: u64 },
    SynthesizeZig { path: String, logic: String },
    GenerateReport { #[serde(flatten)] data: Value },
    ScrapeTarget { url: String },
    QueryMemory { query: String },
    /// Halt: Signals the task loop that the directive is fully complete.
    /// The agent emits this as: {"action": "halt"}
    Halt,
}

impl AgentAction {
    pub fn extract_and_parse(text: &str) -> Result<Vec<AgentAction>> {
        let unescaped = text.replace("&quot;", "\"");
        if let Some(start_idx) = unescaped.rfind("```json") {
            let block = &unescaped[start_idx + 7..];
            if let Some(end_idx) = block.find("```") {
                return serde_json::from_str(block[..end_idx].trim())
                    .context("Failed to parse final JSON block");
            }
        }
        if let (Some(s), Some(e)) = (unescaped.rfind('['), unescaped.rfind(']')) {
            return serde_json::from_str(&unescaped[s..=e]).context("Failed to bind JSON brackets");
        }
        Err(anyhow!("CORTEX FRACTURE: Re-sync JSON required."))
    }
}
