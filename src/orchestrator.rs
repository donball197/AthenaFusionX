// src/orchestrator.rs — v3.1 [SSE_FLUSH_FIXED + TIMEOUT]
use crate::neural_agent::ContextPacket;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::broadcast;

pub struct Orchestrator;

impl Orchestrator {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        prompt: String,
        ctx: ContextPacket,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        let model = ctx.model_override.as_deref().unwrap_or("gemma-4-26b-a4b-it");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        let mut combined_text = String::new();
        if let Some(ref sys) = ctx.system_prompt {
            combined_text.push_str(&format!("SYSTEM DIRECTIVE:\n{}\n\n", sys));
        }
        if let Some(ref content) = ctx.current_content {
            if !content.is_empty() {
                combined_text.push_str(&format!("CONTEXT:\n{}\n\n", content));
            }
        }
        combined_text.push_str(&prompt);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 8192,
                "topP": 0.85
            }
        });

        let mut attempts = 0;
        while attempts < 3 {
            attempts += 1;
            let result = client
                .post(&url)
                .json(&payload)
                .timeout(Duration::from_secs(300))
                .send()
                .await;

            match result {
                Ok(res) if res.status().is_success() => {
                    let mut full_response = String::new();
                    let mut stream = res.bytes_stream();
                    let mut byte_buf: Vec<u8> = Vec::new();

                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => byte_buf.extend_from_slice(&chunk),
                            Err(e) => {
                                eprintln!("[ORCH] Stream read error: {}", e);
                                break;
                            }
                        }

                        // Process all complete lines in the buffer
                        while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                            let line_bytes: Vec<u8> = byte_buf.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&line_bytes);
                            let clean = line.trim_start_matches("data: ").trim();
                            if clean.is_empty() || clean == "[DONE]" {
                                continue;
                            }
                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val
                                    .pointer("/candidates/0/content/parts/0/text")
                                    .and_then(|v| v.as_str())
                                {
                                    full_response.push_str(token);
                                    if let Some(ref sender) = tx {
                                        let _ = sender.send(
                                            json!({"type": "NEURAL_STREAM", "payload": token})
                                                .to_string(),
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Flush any remaining partial line in the buffer
                    if !byte_buf.is_empty() {
                        let line = String::from_utf8_lossy(&byte_buf);
                        let clean = line.trim_start_matches("data: ").trim();
                        if !clean.is_empty() && clean != "[DONE]" {
                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val
                                    .pointer("/candidates/0/content/parts/0/text")
                                    .and_then(|v| v.as_str())
                                {
                                    full_response.push_str(token);
                                }
                            }
                        }
                    }

                    return Ok(full_response);
                }
                Ok(res) => {
                    let status = res.status();
                    eprintln!("[ORCH] HTTP {} on attempt {}", status, attempts);
                    tokio::time::sleep(Duration::from_millis(500 * attempts as u64)).await;
                }
                Err(e) => {
                    eprintln!("[ORCH] Request error on attempt {}: {}", attempts, e);
                    tokio::time::sleep(Duration::from_millis(500 * attempts as u64)).await;
                }
            }
        }
        Err("UPLINK_TIMEOUT: All 3 attempts failed.".into())
    }
}
