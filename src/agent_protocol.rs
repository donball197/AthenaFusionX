// src/agent_protocol.rs - v3.3.0 PROTOCOL
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{Result, anyhow, Context};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    PatchFile { path: String, search: String, replace: String },
    ExecuteCommand { command: String },
    ListDirectory { path: String },
    DeleteFile { path: String },
    CreateDirectory { path: String }, // SUTURE: Integrated new action
    SearchCodebase { pattern: String },
    RunCargo { command: String },
    SpawnDrone { image: String, command: String, mount_path: String },
    SearchReplaceMulti { pattern: String, replace: String, file_pattern: String },
    Message { message: String },
    Stasis { duration_ms: u64 },
    SynthesizeZig { path: String, logic: String },
    GenerateReport { #[serde(flatten)] data: Value },
    ScrapeTarget { url: String },
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
            return serde_json::from_str(&unescaped[s..=e])
                .context("Failed to bind JSON brackets");
        }
        Err(anyhow!("CORTEX FRACTURE: Re-sync JSON required."))
    }
}
