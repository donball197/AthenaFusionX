/**
 * Project Nexus Singularity - Neural Action Extractor (Hardened)
 * v2.1.2 - Supports Multi-line and Array-based Action DNA
 */
use crate::agent_protocol::AgentAction;
use anyhow::{Result, anyhow};
use regex::Regex;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    // ⚡ SUTURE: Added (?s) flag for DotAll mode (matches newlines)
    let re = Regex::new(r"(?s)<ACTION>(.*?)</ACTION>").unwrap();
    let mut actions = Vec::new();

    for cap in re.captures_iter(response) {
        let json_str = cap[1].trim();
        let clean_json = json_str.trim_start_matches("```json")
                                 .trim_start_matches("```")
                                 .trim_end_matches("```")
                                 .trim();

        // ⚡ SUTURE: First attempt to parse as a Vec (Array)
        if let Ok(vec) = serde_json::from_str::<Vec<AgentAction>>(clean_json) {
            actions.extend(vec);
        } 
        // Second attempt to parse as a single AgentAction (Object)
        else if let Ok(single) = serde_json::from_str::<AgentAction>(clean_json) {
            actions.push(single);
        } else {
            println!("!! [PARSER_FAULT] Invalid Action DNA (Syntax Error): {}", clean_json);
        }
    }

    if actions.is_empty() && response.contains("<ACTION>") {
        return Err(anyhow!("PROTOCOL_VIOLATION: Malformed DNA inside tags."));
    }

    Ok(actions)
}
