use crate::agent_protocol::AgentAction;
use anyhow::Result;

/// Public shared JSON string escape sanitizer.
pub fn sanitize_json_escapes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_string = false;
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_string = !in_string;
                out.push('"');
            }
            '\n' if in_string => {
                out.push_str("\\n");
            }
            '\r' if in_string => {
                out.push_str("\\r");
            }
            '\\' => {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' || next == '/' || next == 'b' || next == 'f' || next == 'n' || next == 'r' || next == 't' || next == 'u' {
                        out.push('\\');
                    } else {
                        out.push_str("\\\\");
                    }
                } else {
                    out.push_str("\\\\");
                }
            }
            _ => out.push(c),
        }
    }
    out
}

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();

    if text.is_empty() || text == "[]" {
        return Ok(vec![]);
    }

    let trimmed = text.replace("```json", "").replace("```", "");
    let clean_text = sanitize_json_escapes(trimmed.trim());

    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&clean_text) {
        return Ok(actions);
    }

    if let Ok(single) = serde_json::from_str::<AgentAction>(&clean_text) {
        return Ok(vec![single]);
    }

    if let (Some(start), Some(end)) = (clean_text.find('['), clean_text.rfind(']')) {
        if start < end {
            let slice = &clean_text[start..=end];
            if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(slice) {
                return Ok(actions);
            }
        }
    }

    if let (Some(start), Some(end)) = (clean_text.find('{'), clean_text.rfind('}')) {
        if start < end {
            let slice = &clean_text[start..=end];
            if let Ok(single) = serde_json::from_str::<AgentAction>(slice) {
                return Ok(vec![single]);
            }
        }
    }

    tracing::warn!("[JSON PARSER] Could not parse action payload. Falling back safely.");
    Ok(vec![])
}
