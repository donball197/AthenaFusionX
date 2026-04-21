// src/json_parser.rs — v2.9.0 Robust Owned Parser
use crate::agent_protocol::AgentAction;
use anyhow::{Result, anyhow};
use regex::Regex;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let re = Regex::new(r"(?s)<ACTION>(.*?)</ACTION>").unwrap();
    
    // ⚡ FIX: Convert to owned String immediately to solve "does not live long enough"
    let raw_content = match re.captures_iter(response).last() {
        Some(cap) => cap[1].to_string(),
        None => return Ok(Vec::new()),
    };

    let clean_json = raw_content.trim_start_matches("```json")
                                .trim_start_matches("```")
                                .trim_end_matches("```")
                                .trim();

    // 1. Attempt direct parse
    if let Ok(vec) = serde_json::from_str::<Vec<AgentAction>>(clean_json) {
        return Ok(vec);
    }

    // 2. Resilience: Handle nested escaped quotes from grep/shell outputs
    let mut sanitized = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for c in clean_json.chars() {
        if escaped {
            sanitized.push(c);
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            sanitized.push(c);
            continue;
        }
        if c == '"' { in_string = !in_string; }
        if c == '\n' && in_string {
            sanitized.push_str("\\n");
        } else {
            sanitized.push(c);
        }
    }

    serde_json::from_str::<Vec<AgentAction>>(&sanitized)
        .map_err(|e| anyhow!("JSON_DECODE_FRACTURE: {} | Check the format of your <ACTION> block.", e))
}
