// src/orchestrator.rs — v4.3 SOVEREIGN API BRIDGE
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use futures_util::StreamExt;
use crate::neural_agent::ContextPacket;
use std::time::Duration;

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
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);

        let mut combined_text = String::new();
        if let Some(ref sys) = ctx.system_prompt { combined_text.push_str(&format!("SYSTEM DIRECTIVE:\n{}\n\n", sys)); }
        if let Some(ref content) = ctx.current_content { if !content.is_empty() { combined_text.push_str(&format!("CONTEXT:\n{}\n\n", content)); } }
        
        if !ctx.recent_logs.is_empty() {
            let logs = ctx.recent_logs.join("\n>> ");
            combined_text.push_str(&format!("RECENT OBSERVATIONS (TEAM LOGS):\n>> {}\n\n", logs));
        }
        combined_text.push_str(&prompt);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 8192, "topP": 0.85 }
        });

        if let Some(ref sender) = tx {
            let _ = sender.send(json!({ "type": "NEURAL_THOUGHT", "payload": format!("[UPLINK] → {} | 256K ctx | streaming SSE", model) }).to_string());
        }

        // ⚡ FIX: 3-Strike Resilience Loop for Crostini network drops
        let mut attempts = 0;
        let mut last_err = String::new();

        while attempts < 3 {
            attempts += 1;
            match client.post(&url).json(&payload).timeout(Duration::from_secs(45)).send().await {
                Ok(res) => {
                    if !res.status().is_success() { 
                        return Err(format!("API_FAULT [model={} status={}]: {}", model, res.status(), res.text().await.unwrap_or_default())); 
                    }

                    let mut full_response = String::new();
                    let mut byte_buf = Vec::new();
                    let mut stream = res.bytes_stream();

                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                byte_buf.extend_from_slice(&chunk);
                                while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                                    let line_bytes: Vec<u8> = byte_buf.drain(..=pos).collect();
                                    let clean = String::from_utf8_lossy(&line_bytes).trim_start_matches("data: ").trim().to_string();
                                    if clean.is_empty() || clean == "[DONE]" { continue; }
                                    if let Ok(val) = serde_json::from_str::<Value>(&clean) {
                                        if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                                            full_response.push_str(token);
                                            if let Some(ref sender) = tx { let _ = sender.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string()); }
                                        }
                                    }
                                }
                            }
                            Err(e) => return Err(format!("STREAM_FAULT: {}", e)),
                        }
                    }
                    return Ok(full_response);
                },
                Err(e) => {
                    last_err = format!("NETWORK_FAULT (Attempt {}): {}", attempts, e);
                    if let Some(ref sender) = tx {
                        let _ = sender.send(json!({ "type": "NEURAL_STREAM", "payload": format!("\n\x1b[33m>> [RETRY] Network drop detected. Retrying uplink ({}/3)...\x1b[0m\n", attempts) }).to_string());
                    }
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
        }
        
        Err(last_err)
    }
}
