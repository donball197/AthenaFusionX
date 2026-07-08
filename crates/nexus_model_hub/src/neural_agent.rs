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
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();
        let model = ctx.model_override.as_deref().unwrap_or("gemma-4-31b-it");
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key.trim());

        let combined_text =
            format!("SYSTEM: {}\nUSER: {}", ctx.system_prompt.unwrap_or_default(), user_text);
        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": { "temperature": 0.2, "maxOutputTokens": 8192 }
        });

        let response = client.post(&url).json(&payload).send().await?;
        let mut full_response = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Stream chunk error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find("\n\n") {
                let event = buffer.drain(..pos + 2).collect::<String>();
                let clean = event.trim_start_matches("data: ").trim();

                if clean.is_empty() || clean == "[DONE]" {
                    continue;
                }

                if let Ok(val) = serde_json::from_str::<Value>(clean) {
                    if let Some(parts) =
                        val.pointer("/candidates/0/content/parts").and_then(|v| v.as_array())
                    {
                        for part in parts {
                            if let Some(token) = part["text"].as_str() {
                                full_response.push_str(token);
                                let _ = self.tx.send(
                                    json!({"type": "NEURAL_STREAM", "payload": token}).to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok((full_response, start.elapsed().as_millis()))
    }
}
