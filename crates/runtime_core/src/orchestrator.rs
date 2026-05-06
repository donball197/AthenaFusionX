use crate::neural_agent::ContextPacket;
use crate::metrics::METRICS;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::error;

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

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "systemInstruction": {
                "parts": [{
                    "text": ctx.system_prompt.as_deref().unwrap_or("Output ONLY a JSON array. Zero prose.")
                }]
            },
            "generationConfig": { 
                "temperature": 0.1, 
                "maxOutputTokens": 4096,
            }
        });

        for _attempt in 1..=2u64 {
            let t_start = Instant::now();
            let res = client
                .post(&url)
                .timeout(Duration::from_secs(45))
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(response) if response.status().is_success() => {
                    let mut full_text = String::new();
                    let mut stream = response.bytes_stream();
                    let mut buf: Vec<u8> = Vec::new();

                    while let Some(Ok(chunk)) = stream.next().await {
                        buf.extend_from_slice(&chunk);

                        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                            let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&line_bytes);
                            let clean = line.trim_start_matches("data: ").trim();

                            if clean.is_empty() || clean == "[DONE]" {
                                continue;
                            }

                            if let Ok(val) = serde_json::from_str::<Value>(clean) {
                                if let Some(parts) = val.pointer("/candidates/0/content/parts")
                                    .and_then(|v| v.as_array())
                                {
                                    for part in parts {
                                        // Skip internal thought tokens
                                        if part["thought"].as_bool().unwrap_or(false) {
                                            continue;
                                        }
                                        if let Some(token) = part["text"].as_str() {
                                            full_text.push_str(token);
                                            if let Some(sender) = &tx {
                                                let _ = sender.send(
                                                    json!({"type": "NEURAL_STREAM", "payload": token}).to_string()
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    METRICS.api_latency_ms_sum.fetch_add(
                        t_start.elapsed().as_millis() as u64,
                        Ordering::Relaxed,
                    );

                    let raw = full_text.trim();
                    // Find the LAST line that starts with [ opening a JSON array.
                    // This skips [ chars embedded in prose, code blocks, and reasoning.
                    let raw_lines: Vec<&str> = raw.lines().collect();
                    let json_start = raw_lines.iter().rposition(|l| {
                        let t = l.trim();
                        t.starts_with('[') && t.len() > 1 && {
                            let rest = t[1..].trim_start();
                            rest.starts_with('{') || rest.starts_with(']') || rest == "]"
                        }
                    });
                    let out = if let Some(si) = json_start {
                        let slice = raw_lines[si..].join("
");
                        if let Some(e) = slice.rfind(']') { slice[..=e].to_string() } else { raw.to_string() }
                    } else { raw.to_string() };
                    return Ok(out.trim().to_string());
                }
                Ok(resp) => error!("API ERROR: {}", resp.status()),
                Err(e) => error!("NETWORK ERROR: {}", e),
            }
        }

        Err("CORTEX_UPLINK_FAILED".into())
    }
}
