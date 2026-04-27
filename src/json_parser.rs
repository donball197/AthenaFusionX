use anyhow::{Context, Result};
use serde_json::Value;
use crate::agent_protocol::AgentAction;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut text = response.trim().to_string();

    // Strip common LLM wrappers and tags
    text = text
        .replace("<THOUGHT>", "")
        .replace("</THOUGHT>", "")
        .replace("<ACTION>", "")
        .replace("</ACTION>", "")
        .trim()
        .to_string();

    // Try to find the last JSON array (most robust method)
    if let Some(start) = text.rfind('[') {
        let candidate = &text[start..];
        if let Ok(actions) = serde_json::from_str::<Vec<Value>>(candidate) {
            let parsed: Vec<AgentAction> = actions
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect();
            if !parsed.is_empty() {
                return Ok(parsed);
            }
        }
    }

    // Full parse fallback
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&text) {
        return Ok(actions);
    }

    // Last-ditch effort: any JSON array in the text
    if let Some(pos) = text.find('[') {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&text[pos..]) {
            return Ok(actions);
        }
    }

    Err(anyhow::anyhow!(
        "JSON_DECODE_FRACTURE: Could not extract actions. First 200 chars: {}",
        text.chars().take(200).collect::<String>()
    ))
}
