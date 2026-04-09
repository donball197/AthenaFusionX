// src/neural_agent.rs - v3.5 Visual Cortex Suture (Broadcast Enabled)
use anyhow::{Context, Result};
use serde_json::{json, Value};
use reqwest::Client;
use std::sync::Mutex;
use std::collections::VecDeque;
use tokio::sync::broadcast;
use futures_util::StreamExt;
use std::io::{self, Write};
use crate::metrics::METRICS;

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
        resonance: f32,
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();

        let (temp, max_tokens, thinking_enabled) = if resonance >= 0.8 {
            (0.7, 8192, true)  // Motor/Recovery
        } else if resonance > 0.4 {
            (0.3, 4096, false) // Cognitive
        } else {
            (0.1, 2048, false) // Sensory
        };

        let model = std::env::var("NEXUS_MODEL").unwrap_or_else(|_| "gemma-4-26b-a4b-it".to_string());
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);

        let base_prompt = ctx.system_prompt.unwrap_or_else(|| "You are NEXUS Ghost.".to_string());
        let sys_prompt = if thinking_enabled { format!("<|think|>\n{}", base_prompt) } else { base_prompt };

        let payload = json!({
            "system_instruction": { "parts": [{ "text": sys_prompt }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_text }] }],
            "generationConfig": { "temperature": temp, "maxOutputTokens": max_tokens, "topP": 0.95 }
        });

        println!(">> [NEURAL] Resonance: {:.2} | Thinking: {}", resonance, thinking_enabled);

        let response = client.post(&url).json(&payload).send().await.context("API connection failed")?;
        let mut clean_text = String::new();
        let mut line_buffer = String::new();
        let mut is_thinking = false;
        let mut stream = response.bytes_stream();

        print!("\n\x1b[38;5;51m[NEXUS STREAM] \x1b[0m");

        while let Some(item) = stream.next().await {
            if let Ok(bytes) = item {
                line_buffer.push_str(&String::from_utf8_lossy(&bytes).replace("\r\n", "\n"));
                while let Some(pos) = line_buffer.find("\n\n") {
                    let chunk = line_buffer[..pos].to_string();
                    line_buffer.drain(..pos + 2);
                    let clean = chunk.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" { continue; }

                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                            let mut remaining = token;
                            
                            while !remaining.is_empty() {
                                if !is_thinking {
                                    if let Some(start_pos) = remaining.find("<think>") {
                                        let clean_part = &remaining[..start_pos];
                                        self.broadcast_token(clean_part, false);
                                        print!("\x1b[38;5;253m{}\x1b[0m", clean_part);
                                        clean_text.push_str(clean_part);
                                        
                                        is_thinking = true;
                                        print!("\x1b[38;5;240m<think>");
                                        remaining = &remaining[start_pos + 7..];
                                    } else {
                                        self.broadcast_token(remaining, false);
                                        print!("\x1b[38;5;253m{}\x1b[0m", remaining);
                                        clean_text.push_str(remaining);
                                        remaining = "";
                                    }
                                } else {
                                    if let Some(end_pos) = remaining.find("</think>") {
                                        let thought_part = &remaining[..end_pos];
                                        self.broadcast_token(thought_part, true);
                                        print!("{}", thought_part);
                                        
                                        is_thinking = false;
                                        print!("</think>\x1b[0m");
                                        remaining = &remaining[end_pos + 8..];
                                    } else {
                                        self.broadcast_token(remaining, true);
                                        print!("{}", remaining);
                                        remaining = "";
                                    }
                                }
                            }
                            io::stdout().flush().ok();
                        }
                    }
                }
            }
        }
        println!("\n\x1b[38;5;244m────────────────────────────────\x1b[0m");
        METRICS.record_api_call(start.elapsed().as_millis());

        Ok((clean_text, start.elapsed().as_millis()))
    }

    fn broadcast_token(&self, text: &str, is_thinking: bool) {
        if text.is_empty() { return; }
        let msg = json!({
            "type": "NEURAL_STREAM",
            "payload": {
                "text": text,
                "is_thinking": is_thinking
            }
        }).to_string();
        let _ = self.tx.send(msg);
    }
}
