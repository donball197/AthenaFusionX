use crate::agent_protocol::AgentAction;
use anyhow::Result;

/// Public shared JSON string escape sanitizer.
pub fn sanitize_json_escapes(json_str: &str) -> String {
    let mut sanitized = String::with_capacity(json_str.len() + 128);
    let mut in_string = false;
    let mut last_char = ' ';

    for c in json_str.chars() {
        match c {
            '"' if last_char != '\\' => {
                in_string = !in_string;
                sanitized.push('"');
            }
            '\n' if in_string => sanitized.push_str("\\n"),
            '\r' if in_string => sanitized.push_str("\\r"),
            '\t' if in_string => sanitized.push_str("\\t"),
            '\\' => {
                sanitized.push('\\');
            }
            _ => sanitized.push(c),
        }
        last_char = c;
    }
    sanitized
}

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();
    if text.is_empty() || text == "[]" { return Ok(vec![]); }

    // 1. Aggressively carve out code blocks
    let mut extracted = text;
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            extracted = &text[start + 7 .. start + 7 + end];
        }
    } else if let Some(start) = text.find("```") {
        if let Some(end) = text[start + 3..].find("```") {
            extracted = &text[start + 3 .. start + 3 + end];
        }
    }

    extracted = extracted.trim();

    // 2. Locate boundaries
    let json_str = if let (Some(s), Some(e)) = (extracted.find('['), extracted.rfind(']')) {
        &extracted[s..=e]
    } else if let (Some(s), Some(e)) = (extracted.find('{'), extracted.rfind('}')) {
        &extracted[s..=e]
    } else { return Err(anyhow::anyhow!("No JSON boundaries found in model output.")); };

    let clean_text = sanitize_json_escapes(json_str);

    // ANTI-DEGENERACY GATE
    if clean_text.contains("00000000000000000000") || clean_text.len() > 25_000 {
        tracing::error!("[CORTEX PARSE ABORT] Token Resonance / Runaway block. Returning safe halt.");
        return Ok(vec![]);
    }

    // 3. Serializer & Transparent Fallback Log
    match serde_json::from_str::<Vec<AgentAction>>(&clean_text) {
        Ok(actions) => Ok(actions),
        Err(e1) => {
            match serde_json::from_str::<AgentAction>(&clean_text) {
                Ok(single) => Ok(vec![single]),
                Err(e2) => {
                    let snippet = if clean_text.len() > 400 { &clean_text[..400] } else { &clean_text };
                    tracing::warn!(
                        "[JSON PARSER] Serialization Failure!\nArray Err: {}\nSingle Err: {}\n--- OFFENDING SNIPPET ---\n{}\n-------------------------", 
                        e1, e2, snippet
                    );
                    Err(anyhow::anyhow!("JSON serialization failed. Invalid format."))
                }
            }
        }
    }
}
