/**
 * Project Nexus Singularity - Neural Action Extractor (Finality)
 * v2.7.0 - Context-Aware String Sanitization
 */
use crate::agent_protocol::AgentAction;
use anyhow::{Result, anyhow};
use regex::Regex;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let re = Regex::new(r"(?s)<ACTION>(.*?)</ACTION>").unwrap();
    let caps: Vec<_> = re.captures_iter(response).collect();

    if caps.is_empty() {
        return response.contains("<ACTION>")
            .then(|| Err(anyhow!("PROTOCOL_VIOLATION: Malformed tags.")))
            .unwrap_or(Ok(Vec::new()));
    }

    let raw_json = caps.last().unwrap()[1].trim();
    let clean_json = raw_json.trim_start_matches("```json")
                             .trim_start_matches("```")
                             .trim_end_matches("```")
                             .trim();

    // ⚡ SMART SUTURE: Context-Aware Newline Escaping
    // We only escape newlines if they occur inside double quotes.
    let mut sanitized = String::new();
    let mut in_string = false;
    let mut chars = clean_json.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' { in_string = !in_string; }
        if c == '\n' && in_string {
            sanitized.push_str("\\n");
        } else if c == '\r' {
            continue;
        } else {
            sanitized.push(c);
        }
    }

    // 1. Attempt standard parse
    if let Ok(vec) = serde_json::from_str::<Vec<AgentAction>>(&sanitized) {
        return Ok(vec);
    } 

    // 2. Deep Clean Fallback (Locate Array)
    if let (Some(start), Some(end)) = (sanitized.find('['), sanitized.rfind(']')) {
        let deep_clean = &sanitized[start..=end];
        if let Ok(vec) = serde_json::from_str::<Vec<AgentAction>>(deep_clean) {
            return Ok(vec);
        }
    }

    Err(anyhow!("PROTOCOL_VIOLATION: Strict Schema Mismatch in Sanitized DNA."))
}
