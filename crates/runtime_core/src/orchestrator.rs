use crate::metrics::METRICS;
use crate::neural_agent::ContextPacket;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{error, warn};

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
        let model = ctx.model_override.as_deref().unwrap_or("models/gemma-4-31b-it");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{}:generateContent?key={}",
            model, api_key
        );

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "systemInstruction": {
                "parts": [{ "text": ctx.system_prompt.as_deref().unwrap_or("Output ONLY a JSON array. Zero prose.") }]
            },
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 4096,
                "thinkingConfig": { "thinkingLevel": "high" }
            }
        });

        for attempt in 1..=3u64 {
            let t_start = Instant::now();
            let res =
                client.post(&url).timeout(Duration::from_secs(60)).json(&payload).send().await;

            match res {
                Ok(response) if response.status().is_success() => {
                    let val: Value = response.json().await.map_err(|e| e.to_string())?;
                    let mut full_text = String::new();
                    if let Some(parts) =
                        val.pointer("/candidates/0/content/parts").and_then(|v| v.as_array())
                    {
                        for part in parts {
                            if part["thought"].as_bool().unwrap_or(false) {
                                if let Some(t) = part["text"].as_str() {
                                    if let Some(s) = &tx {
                                        let _ = s.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": t}).to_string());
                                    }
                                }
                                continue;
                            }
                            if let Some(token) = part["text"].as_str() {
                                full_text.push_str(token);
                            }
                        }
                    }
                    METRICS
                        .api_latency_ms_sum
                        .fetch_add(t_start.elapsed().as_millis() as u64, Ordering::Relaxed);
                    let raw = full_text.trim();
                    let raw_lines: Vec<&str> = raw.lines().collect();
                    let json_start = raw_lines.iter().rposition(|l| l.trim().starts_with('['));
                    let out = if let Some(si) = json_start {
                        let slice = raw_lines[si..].join("\n");
                        if let Some(e) = slice.rfind(']') {
                            slice[..=e].to_string()
                        } else {
                            raw.to_string()
                        }
                    } else {
                        raw.to_string()
                    };
                    return Ok(out.trim().to_string());
                }
                Ok(resp) => {
                    error!("API ERROR (Attempt {}): {}", attempt, resp.status());
                    if attempt < 3 {
                        let backoff = 2u64.pow(attempt as u32);
                        warn!(">> 500 Anomaly: Delaying retry for {}s...", backoff);
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                    }
                }
                Err(e) => {
                    error!("NETWORK ERROR (Attempt {}): {}", attempt, e);
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
        Err("CORTEX_UPLINK_FAILED".into())
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}
