use crate::neural_agent::ContextPacket;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::broadcast;

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
        if let Some(ref content) = ctx.current_content {
            if !content.is_empty() { combined_text.push_str(&format!("CONTEXT:\n{}\n\n", content)); }
        }
        combined_text.push_str(&prompt);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 8192, "topP": 0.85 }
        });

        let mut attempts = 0;
        while attempts < 3 {
            attempts += 1;
            match client.post(&url).json(&payload).timeout(Duration::from_secs(30)).send().await {
                Ok(res) if res.status().is_success() => {
                    let mut full_response = String::new();
                    let mut stream = res.bytes_stream();
                    let mut byte_buf = Vec::new();

                    while let Some(Ok(chunk)) = stream.next().await {
                        byte_buf.extend_from_slice(&chunk);
                        while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                            let line_bytes = byte_buf.drain(..=pos).collect::<Vec<u8>>();
                            let line = String::from_utf8_lossy(&line_bytes);
                            
                            // ⚡ SUTURE: Skip malformed SSE noise
                            if !line.starts_with("data: ") { continue; }
                            let clean = line.trim_start_matches("data: ").trim();
                            if clean.is_empty() || clean == "[DONE]" { continue; }

                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                                    full_response.push_str(token);
                                    if let Some(ref sender) = tx {
                                        let _ = sender.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
                                    }
                                }
                            }
                        }
                    }
                    return Ok(full_response);
                }
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
        Err("UPLINK_TIMEOUT".into())
    }
}
