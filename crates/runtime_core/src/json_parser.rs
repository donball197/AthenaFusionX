use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};
use serde_json::Value;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();

    if text.is_empty() || text == "[]" {
        return Ok(vec![]);
    }

    // STRATEGY 1: Strict Markdown Block Extraction (Most Reliable)
    if let Some(json_block) = extract_markdown_json(text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&json_block) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    // STRATEGY 2: Direct Parse
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(text) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    // STRATEGY 3: Largest Bracket Extractor
    if let Some(array_str) = extract_largest_json_array(text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&array_str) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    // STRATEGY 4: Single Object Fallback
    if let Ok(obj) = serde_json::from_str::<Value>(text) {
        if obj.is_object() {
            if let Ok(action) = serde_json::from_value::<AgentAction>(obj) {
                return Ok(vec![action]);
            }
        }
    }

    // STRATEGY 5: Aggressive Cleaning
    let normalized = normalize_action_json(text);
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&normalized) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    Err(anyhow!(
        "PARSER_FRACTURE: Could not extract valid actions.\nPreview:\n{}",
        text.chars().take(300).collect::<String>()
    ))
}

fn extract_markdown_json(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        let after_start = &text[start + 7..];
        if let Some(end) = after_start.find("```") {
            return Some(after_start[..end].trim().to_string());
        }
    }
    None
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
        let mut in_string = false;
        let mut escape = false;

        for (j, c) in text[abs_start..].char_indices() {
            if escape {
                escape = false;
                continue;
            }
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '[' if !in_string => depth += 1,
                ']' if !in_string => {
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
            // Ignore unclosed bracket and keep searching
            i = abs_start + 1;
        }
    }

    if max_len > 15 {
        Some(text[best_start..best_end].to_string())
    } else {
        None
    }
}

fn normalize_action_json(text: &str) -> String {
    let mut s = text.replace("```json", "").replace("```", "").trim().to_string();
    if s.starts_with('{') && !s.starts_with('[') {
        s = format!("[{}]", s);
    }
    s
}
