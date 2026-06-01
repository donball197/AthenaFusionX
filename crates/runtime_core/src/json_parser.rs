// crates/runtime_core/src/json_parser.rs
use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Ultra-resilient extractor for LLM-generated AgentAction arrays.
/// Handles: markdown, thoughts, truncation, extra text, single objects, etc.
pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();

    // Strategy 0: Empty response guard
    if text.is_empty() || text == "[]" {
        return Ok(vec![]);
    }

    // Strategy 1: Direct parse (cleanest case)
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(text) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    // Strategy 2: Extract largest well-formed JSON array (most common case)
    if let Some(array_str) = extract_largest_json_array(text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&array_str) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    // Strategy 3: Single action object fallback
    if let Ok(obj) = serde_json::from_str::<Value>(text) {
        if obj.is_object() {
            if let Ok(action) = serde_json::from_value::<AgentAction>(obj) {
                return Ok(vec![action]);
            }
        }
    }

    // Strategy 4: Aggressive cleaning + normalization
    let normalized = normalize_action_json(text);
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&normalized) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    // Strategy 5: Last resort - try to salvage partial array
    if let Some(salvaged) = salvage_partial_array(text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&salvaged) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    Err(anyhow!(
        "PARSER_FRACTURE: Could not extract valid actions after 5 strategies.\n\
         Preview (first 300 chars):\n{}",
        text.chars().take(300).collect::<String>()
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
            i = e; // Continue searching after this array
        } else {
            break;
        }
    }

    if max_len > 15 {
        Some(text[best_start..best_end].to_string())
    } else {
        None
    }
}

fn normalize_action_json(text: &str) -> String {
    let mut s = text
        .replace("```json", "")
        .replace("```", "")
        .replace("thought:", "\"thought\":") // preserve thoughts if needed
        .replace("action:", "\"type\":")
        .replace("\"action\"", "\"type\"")
        .replace("```", "")
        .trim()
        .to_string();

    // Wrap lone object in array
    if s.starts_with('{') && !s.starts_with('[') {
        s = format!("[{}]", s);
    }

    // Remove common LLM prefixes/suffixes
    s = s
        .trim_start_matches(|c: char| !c.is_ascii_punctuation())
        .trim_end_matches(|c: char| !c.is_ascii_punctuation())
        .to_string();

    s
}

/// Last-ditch effort: close unterminated arrays
fn salvage_partial_array(text: &str) -> Option<String> {
    if let Some(start) = text.find('[') {
        let mut candidate = text[start..].to_string();
        let open = candidate.chars().filter(|&c| c == '[').count();
        let close = candidate.chars().filter(|&c| c == ']').count();

        if open > close {
            candidate.push_str(&"]".repeat(open - close));
        }
        Some(candidate)
    } else {
        None
    }
}
