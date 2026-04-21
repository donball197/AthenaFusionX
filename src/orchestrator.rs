// src/orchestrator.rs — v4.1 SOVEREIGN API BRIDGE
// Copyright 2026 Donald Ball — Apache 2.0
//
// FIX: Model is read from ctx.model_override directly — no more fragile
//      split-on-"Model: " string parsing from system_prompt.
//
// FIX: Uses streamGenerateContent?alt=sse (SSE) instead of the blocking
//      generateContent endpoint. Gemma 4 models require the streaming
//      endpoint for correct generationConfig handling — the blocking
//      endpoint rejects thinkingConfig and certain temperature combos
//      with a 400 INVALID_ARGUMENT on Gemma 4 model names specifically.
//
// FIX: Removed thinkingConfig from the payload entirely — it is not
//      needed for the DevOps FSM use case and was causing the 400 errors
//      seen in the build log when gemma-4-26b-a4b-it was the model.

use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use futures_util::StreamExt;
use crate::neural_agent::ContextPacket;

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
        // Model resolution: ctx field → default MoE
        let model = ctx.model_override
            .as_deref()
            .unwrap_or("gemma-4-26b-a4b-it");

        // SSE streaming endpoint — required for Gemma 4 on the Gemini API
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, api_key
        );

        // Build content parts
        let mut parts: Vec<Value> = Vec::new();
        if let Some(ref sys) = ctx.system_prompt {
            parts.push(json!({ "text": format!("SYSTEM DIRECTIVE:\n{}\n\n", sys) }));
        }
        if let Some(ref content) = ctx.current_content {
            if !content.is_empty() {
                parts.push(json!({ "text": format!("CONTEXT:\n{}\n\n", content) }));
            }
        }
        parts.push(json!({ "text": prompt }));

        // Clean payload — no thinkingConfig, no unsupported fields
        let payload = json!({
            "contents": [{ "role": "user", "parts": parts }],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 8192,
                "topP": 0.85
            }
        });

        if let Some(ref sender) = tx {
            let _ = sender.send(json!({
                "type": "NEURAL_THOUGHT",
                "payload": format!("[UPLINK] → {} | 256K ctx | streaming SSE", model)
            }).to_string());
        }

        let res = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("NETWORK_FAULT: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(format!("API_FAULT [model={} status={}]: {}", model, status, err_text));
        }

        // SSE stream parsing — drain lines, extract token text
        let mut full_response = String::new();
        let mut byte_buf: Vec<u8> = Vec::new();
        let mut stream = res.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    byte_buf.extend_from_slice(&chunk);
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
                Err(e) => return Err(format!("STREAM_FAULT: {}", e)),
            }
        }

        Ok(full_response)
    }
}
