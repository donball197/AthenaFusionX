use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};

/// Streamlined, strict JSON extractor leveraging Native Structured Outputs.
pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();

    if text.is_empty() || text == "[]" {
        return Ok(vec![]);
    }

    // Strip markdown formatting if the model still wrapped it
    let clean_text = text
        .replace("```json", "")
        .replace("```", "")
        .trim()
        .to_string();

    // Since we now rely on strict JSON generation from the LLM, 
    // we bypass the legacy heuristics and directly parse.
    serde_json::from_str::<Vec<AgentAction>>(&clean_text).map_err(|e| {
        anyhow!(
            "PARSER_FRACTURE: Failed to parse strict JSON: {}.\nRaw Data: {}",
            e,
            clean_text.chars().take(300).collect::<String>()
        )
    })
}
