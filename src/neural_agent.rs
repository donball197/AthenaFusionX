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

/// ⚡ FIXED: Properly suppresses <THOUGHT> tags and emits everything else.
pub fn strip_thought_regions(token: &str) -> String {
    if !token.contains('<') {
        return token.to_string();
    }

    let mut out = String::with_capacity(token.len());
    let mut rest = token;
    let mut in_thought = false;

    while !rest.is_empty() {
        if in_thought {
            if let Some(end) = rest.find("</THOUGHT>") {
                rest = &rest[end + 10..];
                in_thought = false;
            } else {
                break;
            }
        } else {
            if let Some(start) = rest.find("<THOUGHT>") {
                out.push_str(&rest[..start]);
                rest = &rest[start + 9..];
                in_thought = true;
            } else {
                out.push_str(rest);
                break;
            }
        }
    }
    out.trim().to_string()
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
        model_override: Option<&str>,
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();
        let model = model_override.unwrap_or("gemma-4-26b-a4b-it");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        let base_prompt = ctx.system_prompt.unwrap_or_else(|| "You are NEXUS Ghost.".to_string());
        let combined_text = format!(
            "{}\n\nCONTEXT:\n{}\n\n{}",
            base_prompt,
            ctx.current_content.unwrap_or_default(),
            user_text
        );

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": {
                "temperature": (resonance * 1.2).clamp(0.1, 0.7),
                "maxOutputTokens": 4096,
                "topP": 0.95
            }
        });

        let response = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("API request failed")?;

        let mut full_response = String::new();
        let mut byte_buffer: Vec<u8> = Vec::new();
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
                        if let Some(token) = val
                            .pointer("/candidates/0/content/parts/0/text")
                            .and_then(|v| v.as_str())
                        {
                            full_response.push_str(token);

                            let visible = strip_thought_regions(token);
                            if !visible.is_empty() {
                                let _ = self.tx.send(
                                    json!({"type": "NEURAL_STREAM", "payload": visible}).to_string(),
                                );
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
