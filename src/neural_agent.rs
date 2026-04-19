use anyhow::{Context, Result};
use serde_json::{json, Value};
use reqwest::Client;
use std::sync::Mutex;
use std::collections::VecDeque;
use tokio::sync::broadcast;
use futures_util::StreamExt;
use crate::metrics::METRICS;

#[derive(Debug, Default, Clone)]
pub struct ContextPacket {
    pub system_prompt: Option<String>,
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
        _model_override: Option<&str>,
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();
        
        // ⚡ SOVEREIGN ALIGNMENT: Fixed Model ID
        let model = "gemma-4-26b-a4b-it";

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);
        
        let base_prompt = ctx.system_prompt.unwrap_or_else(|| "You are NEXUS Ghost.".to_string());
        let combined_text = format!("{}\n\nCONTEXT:\n{}\n\n{}", base_prompt, ctx.current_content.unwrap_or_default(), user_text);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": { 
                "temperature": (resonance * 1.2).clamp(0.1, 1.0), 
                "maxOutputTokens": 8192,
                "topP": 0.98
            }
        });

        let response = client.post(&url).json(&payload).send().await.context("API failed")?;
        let mut full_response = String::new();
        let mut byte_buffer: Vec<u8> = Vec::new();
        let mut is_thinking = false;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            if let Ok(chunk) = chunk_result {
                byte_buffer.extend_from_slice(&chunk);
                while let Some(pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes = byte_buffer.drain(..=pos).collect::<Vec<u8>>();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let clean = line.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" { continue; }
                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                            full_response.push_str(token);
                            
                            if token.contains("<THOUGHT>") { is_thinking = true; }
                            let stream_type = if is_thinking { "NEURAL_THOUGHT" } else { "NEURAL_STREAM" };
                            
                            // ⚡ LIVE-WIRE BROADCAST
                            let _ = self.tx.send(json!({"type": stream_type, "payload": token}).to_string());
                            
                            if token.contains("</THOUGHT>") { is_thinking = false; }
                        }
                    }
                }
            }
        }
        
        METRICS.record_api_call(start.elapsed().as_millis());
        Ok((full_response, start.elapsed().as_millis()))
    }
}
