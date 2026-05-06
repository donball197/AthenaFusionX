import os

# ==============================================================================
# 1. THE WARP DIFF ENGINE (Indentation Agnostic)
# ==============================================================================
diff_code = r"""// src/diff.rs - Secure diff/patch engine — Warp DNA Included
use anyhow::{anyhow, Result};

pub struct DiffEngine;

impl DiffEngine {
    pub fn apply(original: &str, search: &str, replace: &str) -> Result<String> {
        if search.is_empty() {
            return Ok(original.to_string());
        }

        // TIER 1: Exact Match
        if let Some(pos) = original.find(search) {
            let mut result = String::with_capacity(original.len() - search.len() + replace.len());
            result.push_str(&original[..pos]);
            result.push_str(replace);
            result.push_str(&original[pos + search.len()..]);
            return Ok(result);
        }

        // TIER 2: Indentation-Agnostic Match (Warp DNA)
        // Solves the LLM dropping leading whitespace in patches.
        let search_lines: Vec<&str> = search.lines().map(|l| l.trim_start()).collect();
        let orig_lines: Vec<&str> = original.lines().collect();
        let search_len = search_lines.len();

        if search_len > 0 && orig_lines.len() >= search_len {
            for i in 0..=(orig_lines.len() - search_len) {
                let window = &orig_lines[i..i + search_len];
                let window_trimmed: Vec<&str> = window.iter().map(|l| l.trim_start()).collect();
                
                if window_trimmed == search_lines {
                    let mut result = orig_lines[..i].join("\n");
                    if !result.is_empty() { result.push('\n'); }
                    result.push_str(replace);
                    if i + search_len < orig_lines.len() {
                        result.push('\n');
                        result.push_str(&orig_lines[i + search_len..].join("\n"));
                    }
                    return Ok(result);
                }
            }
        }

        Err(anyhow!("PATTERN_NOT_FOUND: Search string could not be matched exactly or agnostically."))
    }

    pub fn unified_diff(_old: &str, _new: &str, path: &str) -> String {
        format!("--- a/{}\n+++ b/{}\n[MODIFICATION APPLIED]", path, path)
    }
}
"""
with open("src/diff.rs", "w") as f: f.write(diff_code)
print("\x1b[32m>> [SUTURE 1] diff.rs (Indentation-Agnostic Engine) deployed.\x1b[0m")

# ==============================================================================
# 2. THE WARP JSON PARSER (Jaro-Winkler Healer)
# ==============================================================================
parser_code = r"""use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use strsim::jaro_winkler;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut text = response.trim().replace("```json", "").replace("```", "").trim().to_string();

    // Standard LLM Muscle Memory fixes
    text = text.replace("\"action\":", "\"type\":");
    text = text.replace("\"file_path\":", "\"path\":");

    // TIER 2: Warp DNA Fuzzy Key Repair
    // If the LLM mangles a key (e.g. "reasonig"), Jaro-Winkler will pull it back into alignment.
    if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(arr) = parsed.as_array_mut() {
            for item in arr.iter_mut() {
                if let Some(obj) = item.as_object_mut() {
                    let keys: Vec<String> = obj.keys().cloned().collect();
                    for k in keys {
                        if k != "type" && k != "reasoning" && k != "path" && k != "content" && k != "command" {
                            if jaro_winkler(&k, "reasoning") > 0.85 {
                                if let Some(v) = obj.remove(&k) { obj.insert("reasoning".to_string(), v); }
                            } else if jaro_winkler(&k, "content") > 0.85 {
                                if let Some(v) = obj.remove(&k) { obj.insert("content".to_string(), v); }
                            }
                        }
                    }
                }
            }
        }
        text = serde_json::to_string(&parsed).unwrap_or(text);
    }

    match serde_json::from_str::<Vec<AgentAction>>(&text) {
        Ok(actions) => Ok(actions),
        Err(e) => Err(anyhow!("SCHEMA_MISMATCH: {}", e))
    }
}
"""
with open("src/json_parser.rs", "w") as f: f.write(parser_code)
print("\x1b[32m>> [SUTURE 2] json_parser.rs (Jaro-Winkler Healer) deployed.\x1b[0m")

# ==============================================================================
# 3. THE WARP TASK MANAGER (Dynamic Project Context)
# ==============================================================================
task_path = "src/task_manager.rs"
with open(task_path, "r") as f:
    task_code = f.read()

# Inject the dynamic workspace rule reader right before the loop
injection = """
    let execution_law = cli::get_execution_law_v6().await;
    
    // ⚡ WARP DNA: Dynamic Project Context Injection
    let mut custom_rules = String::new();
    if let Ok(rules) = tokio::fs::read_to_string(".nexus_rules.md").await {
        custom_rules = format!("\\n\\n=== WORKSPACE RULES ===\\n{}", rules);
        info!("[CONTEXT] Injected .nexus_rules.md into system prompt.");
    }
    
    let combined_prompt = format!("{}{}", execution_law, custom_rules);

    while turn < MAX_TURNS {"""

# Replace the static law assignment
task_code = task_code.replace(
    """    let execution_law = cli::get_execution_law_v6().await;\n    \n    while turn < MAX_TURNS {""",
    injection
)

# Replace the context packet assignment inside the loop
task_code = task_code.replace(
    """system_prompt: Some(execution_law.clone()),""",
    """system_prompt: Some(combined_prompt.clone()),"""
)

with open(task_path, "w") as f: f.write(task_code)
print("\x1b[32m>> [SUTURE 3] task_manager.rs (Dynamic Context) deployed.\x1b[0m")
