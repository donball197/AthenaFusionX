// src/json_parser.rs — v4.3 [CR_AND_UNICODE_ESCAPE_FIXED]
use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};
use regex::Regex;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut raw_content = String::new();

    let re = Regex::new(r"(?s)<ACTION>(.*?)</ACTION>")
        .expect("[PARSER] ACTION regex pattern is invalid");

    if let Some(cap) = re.captures_iter(response).last() {
        if let Some(m) = cap.get(1) {
            raw_content = m.as_str().to_string();
        }
    }

    // Fallback: find the last balanced [...] block
    if raw_content.is_empty() {
        if let Some(end) = response.rfind(']') {
            if let Some(start) = response[..end].rfind('[') {
                raw_content = response[start..=end].to_string();
            }
        }
    }

    if raw_content.is_empty() {
        return Err(anyhow!("No valid <ACTION> block or JSON array found in response."));
    }

    let clean_json = raw_content
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Fast path — try direct parse first
    if let Ok(vec) = serde_json::from_str::<Vec<AgentAction>>(clean_json) {
        return Ok(vec);
    }

    // Sanitise: fix bare newlines and carriage-returns inside JSON strings
    let mut sanitized = String::with_capacity(clean_json.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = clean_json.chars().peekable();

    while let Some(c) = chars.next() {
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
        if c == '"' {
            in_string = !in_string;
        }
        if in_string {
            match c {
                '\n' => sanitized.push_str("\\n"),
                '\r' => sanitized.push_str("\\r"),
                '\t' => sanitized.push_str("\\t"),
                _ => sanitized.push(c),
            }
        } else {
            sanitized.push(c);
        }
    }

    serde_json::from_str::<Vec<AgentAction>>(&sanitized).map_err(|e| {
        let preview = &clean_json[..clean_json.len().min(400)];
        anyhow!("JSON_DECODE_FRACTURE: {} | Raw block:\n{}", e, preview)
    })
}
