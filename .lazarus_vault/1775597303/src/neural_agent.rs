use anyhow::{Context, Result, anyhow};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use reqwest::Client;
use std::sync::Mutex;
use std::collections::VecDeque;
use tokio::sync::broadcast;
use futures_util::StreamExt;
use std::io::{self, Write};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContextPacket {
    pub current_file: String,
    pub recent_logs: Vec<String>,
    pub file_content: Option<String>,
    pub terminal_log: Option<String>,
    pub system_prompt: Option<String>,
    pub target_model: Option<String>,
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn init(tx: broadcast::Sender<String>) -> Result<Self> {
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(16)), tx })
    }

    pub async fn prepare_and_run(
        &self,
        client: &Client,
        api_key: &str,
        user_text: &str,
        ctx: ContextPacket,
        _resonance: f32,
    ) -> Result<(String, u128)> {
        let start_time = std::time::Instant::now();
        let model = "gemma-4-26b-a4b-it";
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", 
            model, api_key
        );

        let sys_prompt = ctx.system_prompt.unwrap_or_else(|| "You are NEXUS Ghost, a Sovereign AI Architect.".to_string());

        let payload = json!({ 
            "system_instruction": { "parts": [{ "text": sys_prompt }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_text }] }],
            "generationConfig": { "temperature": 0.2, "maxOutputTokens": 8192 }
        });

        let _ = self.tx.send(json!({"type": "ghost_start"}).to_string());

        let response = client.post(&url).json(&payload).send().await
            .context("Failed to establish neural uplink to Google API")?;

        if !response.status().is_success() {
            let status = response.status();
            let _ = self.tx.send(json!({"type": "ghost_end"}).to_string());
            return Err(anyhow!("Neural Link Rejected: HTTP {}", status));
        }

        let mut full_text = String::new();
        let mut line_buffer = String::new();
        let mut stream = response.bytes_stream();

        print!("\n\x1b[38;5;51m[NEXUS_STREAM] \x1b[0m");

        while let Some(item) = stream.next().await {
            if let Ok(bytes) = item {
                line_buffer.push_str(&String::from_utf8_lossy(&bytes).replace("\r\n", "\n"));
                while let Some(newline_pos) = line_buffer.find("\n\n") {
                    let chunk = line_buffer[..newline_pos].to_string();
                    line_buffer.drain(..newline_pos + 2);
                    let clean = chunk.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" { continue; }
                    
                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(token) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            print!("\x1b[38;5;253m{}\x1b[0m", token);
                            io::stdout().flush().ok();
                            let _ = self.tx.send(json!({"type": "ghost_token", "payload": token}).to_string());
                            full_text.push_str(token);
                        }
                    }
                }
            }
        }
        
        println!("\n\x1b[38;5;244m--------------------------------\x1b[0m\n");
        let _ = self.tx.send(json!({"type": "ghost_end"}).to_string());
        Ok((full_text, start_time.elapsed().as_millis()))
    }
}
