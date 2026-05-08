use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Robust extractor that handles most LLM output variations
pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();

    // Strategy 1: Direct parse (clean JSON)
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(text) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    // Strategy 2: Extract largest JSON array (handles markdown, extra text, thoughts)
    if let Some(array_str) = extract_largest_json_array(text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&array_str) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    // Strategy 3: Single object fallback
    if let Ok(obj) = serde_json::from_str::<Value>(text) {
        if obj.is_object() {
            if let Ok(action) = serde_json::from_value::<AgentAction>(obj) {
                return Ok(vec![action]);
            }
        }
    }

    // Strategy 4: Aggressive normalization + legacy field support
    let normalized = normalize_action_json(text);
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&normalized) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    Err(anyhow!(
        "PARSER_FRACTURE: Could not extract valid actions after multiple strategies.\nPreview: {:.200}",
        text
    ))
}

fn extract_largest_json_array(text: &str) -> Option<String> {
    let mut best_start = 0;
    let mut best_end = 0;
    let mut max_len = 0;

    let mut i = 0;
    while let Some(start) = text[i..].find('[') {
        let abs_start = i + start;
        let mut depth = 0;
        let mut end = None;

        for (j, c) in text[abs_start..].char_indices() {
            match c {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(abs_start + j + 1);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(e) = end {
            let len = e - abs_start;
            if len > max_len {
                max_len = len;
                best_start = abs_start;
                best_end = e;
            }
            i = e;
        } else {
            break;
        }
    }

    if max_len > 10 {
        Some(text[best_start..best_end].to_string())
    } else {
        None
    }
}

fn normalize_action_json(text: &str) -> String {
    let mut s = text
        .replace("```json", "")
        .replace("```", "")
        .replace("action:", "\"type\":")
        .replace("\"action\"", "\"type\"")
        .trim()
        .to_string();

    // If it starts with { but is not wrapped in [], make it an array
    if s.starts_with('{') && !s.starts_with('[') {
        s = format!("[{}]", s);
    }
    s
}
