// src/agent_protocol.rs
// HARDENED VERSION - Fully synced with Executor schemas
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{Context, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    PatchFile { path: String, search: String, replace: String },
    ExecuteCommand { command: String },
    Message { message: String },
    ListDirectory { path: String },
    DeleteFile { path: String },
    SearchCodebase { pattern: String },
    
    // === ADVANCED ACTIONS ===
    CreateDirectory { path: String },
    RunCargo { command: String },
    GitOperation { operation: String, args: Vec<String> },
    SearchReplaceMulti { pattern: String, replace: String, file_pattern: String },
    RunTests { test_filter: Option<String> },
    GenerateReport { #[serde(flatten)] data: Value },
}

impl AgentAction {
    pub fn extract_and_parse(text: &str) -> Result<Vec<AgentAction>> {
        let text = text.trim();

        // Try direct array
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(text) {
            return Ok(actions);
        }

        // Find JSON array in markdown or text
        let start = text.find('[').unwrap_or(0);
        let end = text.rfind(']').map_or(text.len(), |i| i + 1);
        let json_block = &text[start..end];

        serde_json::from_str::<Vec<AgentAction>>(json_block)
            .with_context(|| format!("Failed to parse action array: {}", json_block))
    }
}
