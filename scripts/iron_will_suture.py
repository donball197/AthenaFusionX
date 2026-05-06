import os

# --- 1. SUTURE: json_parser.rs (The Robust Extractor) ---
parser_code = r"""use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use regex::Regex;
use once_cell::sync::Lazy;

static JSON_ARRAY_RE: Lazy<Regex> = Lazy::new(|| 
    Regex::new(r#"\[\s*\{[\s\S]*?\}\s*(?:,\s*\{[\s\S]*?\}\s*)*\]"#).unwrap()
);

/// Robust extractor for AgentAction arrays from LLM output
pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut text = response.trim().to_string();

    // Aggressive cleanup
    text = text
        .replace("```json", "")
        .replace("```", "")
        .replace("\n", " ")
        .replace("  ", " ")
        .trim()
        .to_string();

    // Fix common model hallucinations
    text = text.replace("\"title\":", "\"path\":");   // sometimes it outputs title instead of path
    text = text.replace("\"file_path\":", "\"path\":");

    // Try 1: Direct parse (cleanest case)
    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&text) {
        if !actions.is_empty() {
            return Ok(actions);
        }
    }

    // Try 2: Regex extract largest array-like block
    if let Some(m) = JSON_ARRAY_RE.find(&text) {
        let candidate = m.as_str();
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(candidate) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    // Try 3: Find any [ ... ] and repair
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        let candidate = &text[start..=end];
        let repaired = repair_json(candidate);
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&repaired) {
            if !actions.is_empty() {
                return Ok(actions);
            }
        }
    }

    Err(anyhow!(
        "PARSER_FRACTURE: Failed to extract valid AgentAction array.\nFirst 800 chars:\n{}",
        text.chars().take(800).collect::<String>()
    ))
}

fn repair_json(text: &str) -> String {
    let mut repaired = text.to_string();

    // Fix common JSON breakage from LLMs
    repaired = repaired.replace(",}", "}");
    repaired = repaired.replace(",]", "]");
    repaired = repaired.replace("}{", "},{");
    repaired = repaired.replace(" ,", ",");

    // Remove stray "title" fields (not in enum)
    if repaired.contains("\"title\"") {
        if let Ok(re) = Regex::new(r#""title"\s*:\s*"[^"]*",?\s*"#) {
            repaired = re.replace_all(&repaired, "").to_string();
        }
    }

    // Ensure it starts and ends correctly
    if !repaired.trim_start().starts_with('[') {
        if let Some(pos) = repaired.find('[') {
            repaired = repaired[pos..].to_string();
        } else {
            repaired = format!("[{}]", repaired);
        }
    }
    if !repaired.trim_end().ends_with(']') {
        repaired.push(']');
    }

    repaired
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repair_title_field() {
        let bad = r#"[ {"type": "write_file", "path": "test.md", "title": "ignore", "content": "hi", "reasoning": "test"} ]"#;
        let repaired = repair_json(bad);
        assert!(repaired.contains("write_file"));
        assert!(!repaired.contains("\"title\""));
    }
}
"""
with open("src/json_parser.rs", "w") as f: f.write(parser_code)
print("\x1b[32m>> [SUTURE] json_parser.rs (Robust Extractor) deployed.\x1b[0m")

# --- 2. SUTURE: cli.rs (Robust JSON Law) ---
cli_code = r"""// src/cli.rs — v4.9 [ROBUST_JSON_LAW]

use crate::task_manager;
use std::io::{self, Write};
use std::sync::Arc;

pub async fn inject_workspace_dna(directive: &str) -> String {
    directive.trim().to_string()
}

pub async fn get_execution_law_v6() -> String {
    r#"You are NEXUS — a precise, rule-bound autonomous coding agent.

=== ABSOLUTE OUTPUT RULES (NEVER VIOLATE) ===
- You MUST respond with **ONLY** a valid JSON array of AgentAction objects.
- Nothing else: no explanations, no markdown, no thoughts, no ```json blocks.
- The response must start with `[` and end with `]`.
- Every object MUST contain "type" and "reasoning".
- Valid "type" values: "read_file", "write_file", "execute_command", "halt", "search_memory", "export_report", "fetch_ui_snapshot"
- For write_file ALWAYS use "path" and "content" (never "title").
- Use "halt" ONLY when the ENTIRE mission is complete. Do NOT use halt to "wait" for results — the system will feed results back in the next turn.

Correct example:
[
  {
    "type": "read_file",
    "path": "src/main.rs",
    "reasoning": "Reading source to identify current version"
  },
  {
    "type": "write_file",
    "path": "VERSION_CHECK.md",
    "content": "NEXUS Singularity v3.8.0 - 2026-04-29",
    "reasoning": "Saving version information"
  },
  {
    "type": "halt",
    "reasoning": "Version check mission completed"
  }
]

Now execute the user directive following these rules exactly."#.to_string()
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    loop {
        print!("\n\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        io::stdout().flush().ok();
        let mut input = String::new();
        if stdin.read_line(&mut input)? == 0 { break; }
        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }

        let mut rx = state.tx.subscribe();
        let stream_handle = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                    if val["type"] == "NEURAL_STREAM" {
                        if let Some(token) = val["payload"].as_str() {
                            print!("\x1b[38;5;33m{}\x1b[0m", token);
                            std::io::stdout().flush().ok();
                        }
                    }
                }
            }
        });

        task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            trimmed,
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
            state.dist.clone(),
        ).await;

        stream_handle.abort();
    }
    Ok(())
}
"""
with open("src/cli.rs", "w") as f: f.write(cli_code)
print("\x1b[32m>> [SUTURE] cli.rs (Robust JSON Law) deployed.\x1b[0m")

# --- 3. SUTURE: orchestrator.rs (Relaxed Schema) ---
orch_code = r"""// src/orchestrator.rs — v2.8 [RELAXED_SCHEMA + ROBUST]

use crate::neural_agent::ContextPacket;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::info;

pub struct Orchestrator;

impl Orchestrator {
    pub fn new() -> Self { Self }

    pub async fn run(
        &self,
        prompt: String,
        ctx: ContextPacket,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        let model = ctx.model_override.as_deref().unwrap_or("gemma-4-26b-a4b-it");
        let url = format!(
            "[https://generativelanguage.googleapis.com/v1beta/models/](https://generativelanguage.googleapis.com/v1beta/models/){}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        let mut contents: Vec<Value> = Vec::new();
        
        if !ctx.recent_logs.is_empty() {
            let obs_block = ctx.recent_logs.join("\n");
            contents.push(json!({
                "role": "user",
                "parts": [{ "text": format!("[ENVIRONMENT OBSERVATIONS]\n{}", obs_block) }]
            }));
            contents.push(json!({
                "role": "model",
                "parts": [{ "text": "[ACKNOWLEDGED]" }]
            }));
        }
        
        contents.push(json!({
            "role": "user",
            "parts": [{ "text": prompt }]
        }));

        let system_instruction = ctx.system_prompt.as_deref().map(|s| json!({
            "parts": [{ "text": s }]
        }));

        // Relaxed config — strict schema was causing model to break JSON
        let generation_config = json!({
            "temperature": 0.0,
            "topP": 0.95,
            "maxOutputTokens": 2048,
            "responseMimeType": "application/json"
            // responseSchema removed — gemma-26b struggles with it
        });

        let mut payload = json!({
            "contents": contents,
            "generationConfig": generation_config
        });
        
        if let Some(sys) = system_instruction {
            payload["systemInstruction"] = sys;
        }

        let mut attempts = 0;
        while attempts < 4 {
            attempts += 1;
            info!("[ORCHESTRATOR] Attempt {} → {} ", attempts, model);

            match client.post(&url).json(&payload).send().await {
                Ok(response) if response.status().is_success() => {
                    let mut full = String::new();
                    let mut stream = response.bytes_stream();
                    let mut buf: Vec<u8> = Vec::new();

                    while let Some(Ok(chunk)) = stream.next().await {
                        buf.extend_from_slice(&chunk);
                        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                            let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&line_bytes);
                            let clean = line.trim_start_matches("data: ").trim();

                            if clean.is_empty() || clean == "[DONE]" { continue; }

                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                                    full.push_str(token);
                                    if let Some(sender) = &tx {
                                        let _ = sender.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
                                    }
                                }
                            }
                        }
                    }
                    if !full.trim().is_empty() {
                        return Ok(full.trim().to_string());
                    }
                }
                _ => {
                    info!("[ORCHESTRATOR] Attempt {} failed, retrying...", attempts);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
        Err("UPLINK_FAILED after retries".to_string())
    }
}
"""
with open("src/orchestrator.rs", "w") as f: f.write(orch_code)
print("\x1b[32m>> [SUTURE] orchestrator.rs (Relaxed Schema) deployed.\x1b[0m")

# --- 4. SUTURE: task_manager.rs (Aggressive Feedback) ---
fpath_task = "src/task_manager.rs"
if os.path.exists(fpath_task):
    with open(fpath_task, "r") as f:
        code = f.read()

    # Find the generic error message and replace it with the aggressive version
    target = 'session_obs.push("CRITICAL: Your previous output was not a valid JSON array of AgentActions. Output ONLY the JSON array next time.".to_string());'
    replacement = 'session_obs.push(format!("Turn {} | PARSER_ERROR: {}. CRITICAL: Output ONLY a clean JSON array of AgentActions next turn. No extra text.", turn, e));'
    
    if target in code:
        code = code.replace(target, replacement)
        with open(fpath_task, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] task_manager.rs error feedback upgraded.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] task_manager.rs already upgraded.\x1b[0m")

