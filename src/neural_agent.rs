use anyhow::Result;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Instant;
use std::sync::Mutex;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use crate::polyglot;
use crate::podman::PodmanEngine;

#[derive(Default, Serialize, Deserialize)]
pub struct ContextPacket {
    pub file_content: Option<String>,
    pub terminal_log: Option<String>, 
    pub memory_recall: Option<String>,
    pub system_stats: Option<String>, 
    pub image_b64: Option<String>, 
}

pub struct MicroAgent { memory: Mutex<VecDeque<Value>> }

impl MicroAgent {
    pub fn init() -> Result<Self> { 
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(16)) }) 
    }

    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str, context: ContextPacket, _resonance: f32) -> Result<(String, u128)> {
        let start = Instant::now();
        let prompt_lower = user_text.to_lowercase();

        // 1. INTELLIGENT ROUTING
        // We look for specific triggers to downgrade tools and upgrade reasoning.
        let (model_endpoint, iq_level, max_loops) = if prompt_lower.contains("apex audit") {
            ("gemini-2.5-pro", "APEX AUDITOR (Tier 4)", 1)
        } else if prompt_lower.contains("fix_error") || prompt_lower.contains("fix this") || prompt_lower.contains("refactor") {
            // TIER 3: SURGICAL CODER. Max reasoning, NO tools, STRICT output.
            ("gemma-3-27b-it", "SURGICAL CODER (Tier 3)", 1)
        } else {
            // TIER 2: GENERALIST. Has tools, can loop.
            ("gemini-2.5-flash-lite", "AUTONOMOUS CONDUCTOR (Tier 2)", 2)
        };

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model_endpoint, api_key);
        println!(">> [ROUTER] Engaged: {} | Target: {}", iq_level, model_endpoint);

        // 2. DEFINE TOOLS (Only used by Tier 2)
        let tools = json!([{
            "functionDeclarations": [
                {
                    "name": "execute_bash",
                    "description": "Native shell access.",
                    "parameters": { "type": "OBJECT", "properties": { "command": { "type": "STRING" } }, "required": ["command"] }
                },
                {
                    "name": "write_file_direct",
                    "description": "Writes file to disk. Use ONLY for background tasks.",
                    "parameters": { "type": "OBJECT", "properties": { "path": { "type": "STRING" }, "content": { "type": "STRING" } }, "required": ["path", "content"] }
                },
                {
                    "name": "spawn_drone",
                    "description": "Summons Podman containers.",
                    "parameters": { "type": "OBJECT", "properties": { "drone_type": { "type": "STRING" } }, "required": ["drone_type"] }
                }
            ]
        }]);

        let project_map = polyglot::generate_project_tree();
        
        // 3. ADAPTIVE SYSTEM PROMPT
        let system_instruction = if max_loops == 1 {
            // TIER 3 / 4: STRICT CODING MODE (No Tools)
            // This prompt is optimized for the auto-healer to easily parse the output.
            format!(
                "You are NEXUS (Mode: {}).
                 CRITICAL PROTOCOL:
                 1. You have NO TOOLS. You cannot write files directly.
                 2. You MUST output the SOLUTION CODE inside a Markdown block: ```rust ... ``` or ```javascript ... ```
                 3. Provide the COMPLETE file content. Do not use placeholders like '// ... rest of code'.
                 4. Focus strictly on fixing the error. 
                 5. DO NOT CHAT. DO NOT EXPLAIN. JUST OUTPUT THE CODE BLOCK.
                 PROJECT MAP:\n{}",
                iq_level, project_map
            )
        } else {
            // TIER 2: AUTONOMOUS MODE (Tools Allowed)
            format!(
                "You are NEXUS (Mode: {}).
                 PROTOCOL:
                 1. Use tools (execute_bash, write_file_direct) to solve problems autonomously.
                 2. If you fix a file using write_file_direct, confirm with 'File patched'.
                 PROJECT MAP:\n{}",
                iq_level, project_map
            )
        };

        let mut current_history: Vec<Value> = { self.memory.lock().unwrap().iter().cloned().collect() };
        let mut enriched_prompt = format!("USER INSTRUCTION: {}\n", user_text);
        if let Some(code) = context.file_content { enriched_prompt.push_str(&format!("\n[ACTIVE FILE]\n{}\n", code)); }
        if let Some(log) = context.terminal_log { enriched_prompt.push_str(&format!("\n[ERROR LOGS]\n{}\n", log)); }
        current_history.push(json!({"role": "user", "parts": [{"text": enriched_prompt}]}));

        let mut final_response = String::new();
        let mut loop_count = 0;

        while loop_count < max_loops {
            let payload = json!({
                "systemInstruction": { "parts": [{ "text": system_instruction }] },
                "contents": current_history,
                // CRITICAL FIX: Only provide tools if loops > 1. If Tier 3, send empty tools list.
                "tools": if max_loops > 1 { tools.clone() } else { json!([]) },
                "generationConfig": { "temperature": 0.2 }
            });

            let resp = client.post(&url).json(&payload).send().await?;
            let body: Value = resp.json().await?;
            
            let part = match body.get("candidates").and_then(|c| c.get(0)).and_then(|c| c.get("content")).and_then(|c| c.get("parts")).and_then(|p| p.get(0)) {
                Some(p) => p,
                None => { break; }
            };
            
            if let Some(func_call) = part.get("functionCall") {
                let name = func_call["name"].as_str().unwrap_or("");
                let args = &func_call["args"];
                let mut tool_output = String::new();
                
                if name == "execute_bash" {
                    let cmd = args["command"].as_str().unwrap_or("");
                    println!(">> [AUTONOMY] Exec: {}", cmd);
                    let out = Command::new("bash").arg("-c").arg(cmd).output().await?;
                    tool_output = format!("STDOUT: {}\nSTDERR: {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                } else if name == "write_file_direct" {
                    let path = args["path"].as_str().unwrap_or("");
                    println!(">> [AUTONOMY] Patch: {}", path);
                    std::fs::write(path, args["content"].as_str().unwrap_or(""))?;
                    tool_output = format!("File updated successfully.");
                } else if name == "spawn_drone" {
                    let d_type = args["drone_type"].as_str().unwrap_or("");
                    match PodmanEngine::spawn_drone(d_type) {
                        Ok(id) => tool_output = format!("Drone online. ID: {}", id),
                        Err(e) => tool_output = format!("Error: {}", e),
                    }
                }

                current_history.push(json!({ "role": "model", "parts": [part.clone()] }));
                current_history.push(json!({
                    "role": "function",
                    "parts": [{ "functionResponse": { "name": name, "response": { "result": tool_output } } }]
                }));
                
                loop_count += 1;
                continue; 
            } 
            
            if let Some(text) = part.get("text") {
                final_response = text.as_str().unwrap_or("Task complete.").to_string();
                current_history.push(json!({"role": "model", "parts": [{"text": &final_response}]}));
                break;
            }
        }

        {
            let mut mem = self.memory.lock().unwrap();
            *mem = current_history.into_iter().collect();
            while mem.len() > 16 { mem.pop_front(); }
        }

        Ok((final_response, start.elapsed().as_millis()))
    }
}
