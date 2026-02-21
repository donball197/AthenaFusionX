use anyhow::Result;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Instant;
use std::sync::Mutex;
use std::collections::VecDeque;

pub struct ContextPacket {
    pub file_content: Option<String>,
    pub terminal_log: Option<String>, 
    pub memory_recall: Option<String>,
    pub system_stats: Option<String>, 
    pub image_b64: Option<String>, 
}

pub struct MicroAgent { memory: Mutex<VecDeque<String>> }

impl MicroAgent {
    pub fn init() -> Result<Self> { Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(6)) }) }

    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str, context: ContextPacket) -> Result<(String, u128)> {
        // CLEAN URL - No Markdown Brackets
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
        
        let system_instruction = "You are NEXUS, a sovereign AI engineer. PROTOCOL: 1. Rust/Axum/Tokio only. 2. Output FULL files starting with 'FILE: path'. 3. Style: Terse/Tactical. 4. Infrastructure: You control local/remote Podman drones.";

        let mut history_string = String::new();
        { let history = self.memory.lock().unwrap(); for msg in history.iter() { history_string.push_str(msg); history_string.push_str("\n"); } }

        let mut context_block = String::new();
        if let Some(stats) = context.system_stats { context_block.push_str(&format!("\n[VITALS]\n{}\n", stats)); }
        if let Some(code) = context.file_content { context_block.push_str(&format!("\n[ACTIVE FILE]\n{}\n", code)); }

        let text_part = json!({ "text": format!("{}\n\nCONTEXT:\n{}\n\nHISTORY:\n{}\n\nUSER: {}", system_instruction, context_block, history_string, user_text) });
        let mut parts = vec![text_part];

        if let Some(img_data) = context.image_b64 {
            let clean_b64 = if let Some(idx) = img_data.find(',') { img_data[idx+1..].to_string() } else { img_data };
            parts.push(json!({ "inline_data": { "mime_type": "image/jpeg", "data": clean_b64 } }));
        }

        let payload = json!({ "contents": [{ "parts": parts }], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 4096 } });
        let start = Instant::now();
        let resp = client.post(url).json(&payload).send().await?;
        let duration = start.elapsed().as_millis();
        let body: Value = resp.json().await?;
        
        let text = body["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("... [SIGNAL LOST] ...").to_string();

        { 
            let mut history = self.memory.lock().unwrap(); 
            history.push_back(format!("USER: {}", user_text)); 
            history.push_back(format!("NEXUS: {}", text)); 
            while history.len() > 6 { history.pop_front(); } 
        }
        Ok((text, duration))
    }
}
