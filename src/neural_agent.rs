// src/neural_agent.rs - Cleaned & Hardened (Safe Parsing Restored)
use anyhow::{Context, Result};
use serde_json::Value;
use reqwest::Client;
use std::sync::Mutex;
use std::collections::VecDeque;
use tokio::sync::broadcast;
use futures_util::StreamExt;
use std::io::{self, Write};

#[derive(Debug, Default, Clone)]
pub struct ContextPacket {
    pub system_prompt: Option<String>,
    pub current_file: String,
    pub recent_logs: Vec<String>,
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn init(tx: broadcast::Sender<String>) -> Result<Self> {
        Ok(MicroAgent {
            memory: Mutex::new(VecDeque::with_capacity(16)),
            tx,
        })
    }

    pub async fn prepare_and_run(
        &self,
        client: &Client,
        api_key: &str,
        user_text: &str,
        ctx: ContextPacket,
        _resonance: f32,
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();
        let model = "gemma-4-26b-a4b-it";
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        let sys_prompt = ctx.system_prompt.unwrap_or_else(|| 
            "You are NEXUS Ghost — Sovereign AI Architect.".to_string()
        );

        let payload = serde_json::json!({
            "system_instruction": { "parts": [{ "text": sys_prompt }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_text }] }],
            "generationConfig": { 
                "temperature": 0.1, 
                "maxOutputTokens": 8192, 
                "topP": 0.95 
            }
        });

        let response = client.post(&url).json(&payload).send().await
            .context("Failed to connect to Gemini API")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("API rejected request: HTTP {}", response.status()));
        }

        let mut full_text = String::new();
        let mut line_buffer = String::new();
        let mut stream = response.bytes_stream();

        print!("\n\x1b[38;5;51m[NEXUS STREAM] \x1b[0m");
        
        // SUTURE: Reverted to serde_json parser to safely unescape quotes in tool blocks
        while let Some(item) = stream.next().await {
            if let Ok(bytes) = item {
                line_buffer.push_str(&String::from_utf8_lossy(&bytes).replace("\r\n", "\n"));
                while let Some(newline_pos) = line_buffer.find("\n\n") {
                    let chunk = line_buffer[..newline_pos].to_string();
                    line_buffer.drain(..newline_pos + 2);
                    let clean = chunk.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" { continue; }
                    
                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                            print!("\x1b[38;5;253m{}\x1b[0m", token);
                            io::stdout().flush().ok();
                            let _ = self.tx.send(serde_json::json!({"type": "ghost_token", "payload": token}).to_string());
                            full_text.push_str(token);
                        }
                    }
                }
            }
        }
        println!("\n\x1b[38;5;244m────────────────────────────────\x1b[0m");

        Ok((full_text, start.elapsed().as_millis()))
    }
}
