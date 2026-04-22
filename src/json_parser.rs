// src/json_parser.rs — v3.0 Robust Owned Parser
// FIX: Regex::new(...).unwrap() → compile-time panic-safe expect().
// FIX: caps.last().unwrap() → safe match with early return.
use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};
use regex::Regex;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    // Regex is a compile-time constant pattern — expect() gives a clear panic
    // message at startup if the pattern is ever accidentally mutated.
    let re = Regex::new(r"(?s)<ACTION>(.*?)</ACTION>")
        .expect("[PARSER] ACTION regex pattern is invalid — this is a compile-time bug");

    // FIX: caps.last() instead of .captures() to get the final ACTION block.
    // Using ok_or returns an empty vec (no actions) rather than panicking.
    let raw_content = match re.captures_iter(response).last() {
        Some(cap) => {
            // cap[1] is the captured group; using .get(1) avoids the index panic.
            match cap.get(1) {
                Some(m) => m.as_str().to_string(),
                None => return Ok(Vec::new()),
            }
        }
        None => return Ok(Vec::new()),
    };

    let clean_json = raw_content
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // 1. Direct parse
    if let Ok(vec) = serde_json::from_str::<Vec<AgentAction>>(clean_json) {
        return Ok(vec);
    }

    // 2. Resilience: sanitise escaped newlines inside JSON strings
    let mut sanitized = String::with_capacity(clean_json.len());
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
        if c == '"' {
            in_string = !in_string;
        }
        if c == '\n' && in_string {
            sanitized.push_str("\\n");
        } else {
            sanitized.push(c);
        }
    }

    serde_json::from_str::<Vec<AgentAction>>(&sanitized).map_err(|e| {
        anyhow!(
            "JSON_DECODE_FRACTURE: {} | Raw block was:\n{}",
            e,
            &clean_json[..clean_json.len().min(400)]
        )
    })
}
