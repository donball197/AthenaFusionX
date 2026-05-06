use crate::metrics::METRICS;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::broadcast;

#[derive(Debug, Default, Clone)]
pub struct ContextPacket {
    pub system_prompt: Option<String>,
    pub model_override: Option<String>,
    pub current_file: String,
    pub current_content: Option<String>,
    pub recent_logs: Vec<String>,
    pub task_complexity: u8,
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
        model_override: Option<&str>,
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();
        let model = model_override.or(ctx.model_override.as_deref()).unwrap_or("gemma-4-26b-a4b-it");
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);

        let combined_text = format!("SYSTEM: {}\nUSER: {}", ctx.system_prompt.unwrap_or_default(), user_text);
        
        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 4096 }
        });

        let response = client.post(&url).json(&payload).send().await.context("API request failed")?;
        let mut full_response = String::new();
        let mut byte_buffer = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            if let Ok(chunk) = chunk_result {
                byte_buffer.extend_from_slice(&chunk);
                while let Some(pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = byte_buffer.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let clean = line.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" { continue; }

                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(parts) = val.pointer("/candidates/0/content/parts").and_then(|v| v.as_array()) {
                            for part in parts {
                                // ⚡ CRITICAL: Filter out Gemma 4 reasoning/thought parts
                                if part["thought"].as_bool().unwrap_or(false) { continue; }
                                
                                if let Some(token) = part["text"].as_str() {
                                    full_response.push_str(token);
                                    let _ = self.tx.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        METRICS.record_api_call(start.elapsed().as_millis());
        Ok((full_response, start.elapsed().as_millis()))
    }
}
