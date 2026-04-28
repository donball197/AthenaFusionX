use crate::neural_agent::ContextPacket;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, error};

pub struct Orchestrator;

impl Orchestrator {
    pub fn new() -> Self { Self }

    pub async fn run(&self, prompt: String, ctx: ContextPacket, _: &Client, api_key: &str, tx: Option<broadcast::Sender<String>>) -> Result<String, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(240))
            .build().map_err(|e| e.to_string())?;

        let model = ctx.model_override.as_deref().unwrap_or("gemma-4-26b-a4b-it");
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": 0.55,
                "topP": 0.90,
                "topK": 40,
                "maxOutputTokens": 2048,
                "responseMimeType": "application/json"
            }
        });

        let mut attempts = 0;
        while attempts < 3 {
            attempts += 1;
            info!("[ORCHESTRATOR] Attempt {} → {}...", attempts, model);
            let res = client.post(&url).json(&payload).send().await;

            match res {
                Ok(response) if response.status().is_success() => {
                    let mut full = String::new();
                    let mut stream = response.bytes_stream();
                    let mut buf = Vec::new();
                    while let Some(Ok(chunk)) = stream.next().await {
                        buf.extend_from_slice(&chunk);
                        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                            let line = String::from_utf8_lossy(&buf.drain(..=pos).collect::<Vec<_>>());
                            let clean = line.trim_start_matches("data: ").trim();
                            if clean.is_empty() || clean == "[DONE]" { continue; }
                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                                    full.push_str(token);
                                    if let Some(s) = &tx { let _ = s.send(json!({"type":"NEURAL_STREAM","payload":token}).to_string()); }
                                }
                            }
                        }
                    }
                    return Ok(full);
                }
                Ok(r) => error!("[ORCH] API Error {}: {}", r.status(), r.text().await.unwrap_or_default()),
                Err(e) => error!("[ORCH] Transport Error: {}", e),
            }
            tokio::time::sleep(Duration::from_millis(1500 * attempts as u64)).await;
        }
        Err("UPLINK_TIMEOUT".into())
    }
}
