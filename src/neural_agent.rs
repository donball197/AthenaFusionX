use anyhow::Result;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Instant;
use std::sync::Mutex;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct ContextPacket {
    pub file_content: Option<String>,
    pub terminal_log: Option<String>, 
    pub memory_recall: Option<String>,
    pub system_stats: Option<String>, 
    pub image_b64: Option<String>, 
}

pub struct MicroAgent { memory: Mutex<VecDeque<String>> }

impl MicroAgent {
    pub fn init() -> Result<Self> { 
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(8)) }) 
    }

    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str, context: ContextPacket, resonance: f32) -> Result<(String, u128)> {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
        
        let temperature = 0.1 + (resonance * 0.8);
        let mood = if resonance > 0.7 { "ARCHITECT" } else { "ENGINEER" };
        
        let system_instruction = format!(
            "You are NEXUS (Alpine Edition). Resonance: {:.2} (Mode: {}).
             PROTOCOL:
             0. SPELLING: If you see 'not found in this scope', check for typos in variable and function names against the provided context.
             1. CODE: Rust/Axum. CRITICAL: Always output the ENTIRE file content. Never truncate. Output with 'FILE: path'.
             2. HIVE: To create drones, use:
                DOCKERFILE: name
                ```dockerfile
                FROM alpine:latest
                RUN apk add ...
                ```
             3. CONSTRAINT: Prefer Alpine Linux. Keep it small.",
            resonance, mood
        );

        let mut history_string = String::new();
        { 
            let history = self.memory.lock().unwrap(); 
            for msg in history.iter() { history_string.push_str(msg); history_string.push_str("\n"); } 
        }

        let mut context_block = String::new();
        if let Some(stats) = context.system_stats { context_block.push_str(&format!("\n[VITALS]\n{}\n", stats)); }
        
        // SMART TRUNCATION
        if let Some(code) = context.file_content { 
            let lines: Vec<&str> = code.lines().collect();
            if lines.len() > 1000 {
                let head = lines[..200].join("\n");
                let tail = lines[lines.len()-200..].join("\n");
                context_block.push_str(&format!("\n[ACTIVE_FILE (TRUNCATED)]\n{}\n... [{} lines hidden] ...\n{}\n", head, lines.len()-400, tail));
            } else {
                context_block.push_str(&format!("\n[ACTIVE]\n{}\n", code)); 
            }
        }
        
        if let Some(log) = context.terminal_log { context_block.push_str(&format!("\n[LOGS]\n{}\n", log)); }

        let text_part = json!({ 
            "text": format!("{}\n\nCONTEXT:\n{}\n\nHISTORY:\n{}\n\nUSER: {}", system_instruction, context_block, history_string, user_text) 
        });
        
        let mut parts = vec![text_part];
        if let Some(img_data) = context.image_b64 {
            let clean_b64 = if let Some(idx) = img_data.find(',') { img_data[idx+1..].to_string() } else { img_data };
            parts.push(json!({ "inline_data": { "mime_type": "image/jpeg", "data": clean_b64 } }));
        }

        let payload = json!({ 
            "contents": [{ "parts": parts }], 
            "generationConfig": { "temperature": temperature, "maxOutputTokens": 8192 } 
        });
        
        let start = Instant::now();
        let resp = client.post(url).json(&payload).send().await?;
        let duration = start.elapsed().as_millis();
        
        let body: Value = resp.json().await?;
        let text = body["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("...").to_string();

        { 
            let mut history = self.memory.lock().unwrap(); 
            history.push_back(format!("U: {}", user_text)); 
            history.push_back(format!("N: {}", text)); 
            while history.len() > 8 { history.pop_front(); } 
        }
        Ok((text, duration))
    }
}
