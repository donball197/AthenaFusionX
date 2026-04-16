/**
 * Copyright 2026 Donald Ball
 * Project Nexus Singularity - v1.9.6 (Cognitive Loop Failsafe)
 */

use anyhow::{Context, Result};
use serde_json::{json, Value};
use reqwest::Client;
use std::sync::Mutex;
use std::collections::VecDeque;
use tokio::sync::broadcast;
use futures_util::StreamExt;
use std::io::{self, Write};
use crate::metrics::METRICS;

#[derive(Debug, Default, Clone)]
pub struct ContextPacket {
    pub system_prompt: Option<String>,
    pub current_file: String,
    pub current_content: Option<String>,
    pub recent_logs: Vec<String>,
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
        model_override: Option<&str>, // SUTURE: Dynamic model selection
    ) -> Result<(String, u128)> {
        let start = std::time::Instant::now();

        let (temp, max_tokens, ___thinking_enabled) = if resonance >= 0.8 {
            (0.7, 8192, true)
        } else if resonance > 0.4 {
            (0.3, 4096, false)
        } else {
            (0.1, 2048, false)
        };

        // If no override, fall back to environment or default
        let model = model_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| std::env::var("NEXUS_MODEL").unwrap_or_else(|_| "gemma-4-26b-a4b-it".to_string()));
            
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);

        // API GOVERNOR: Hardened baseline prompt to reject ANSI and raw code repetition
        let base_prompt = ctx.system_prompt.unwrap_or_else(|| "You are NEXUS Ghost. CRITICAL CONSTRAINT: Do not quote raw source code, strings, or ANSI escape sequences (\\x1b...) in your response. Output pure analytical reasoning and strict JSON.".to_string());
        
        let combined_text = if let Some(content) = ctx.current_content {
            format!("{}\n\nCONTEXT:\n{}\n\n{}", base_prompt, content, user_text)
        } else {
            format!("{}\n\n{}", base_prompt, user_text)
        };

        // API GOVERNOR: Injection of presence and frequency penalties to mathematically break loops
        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": combined_text }] }],
            "generationConfig": { 
                "temperature": temp, 
                "maxOutputTokens": max_tokens, 
                "topP": 0.95,
            }
        });

        println!(">> [NEURAL] Model: {} | Resonance: {:.2}", model, resonance);

        let response = client.post(&url).json(&payload).send().await.context("API connection failed")?;
        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_default();
            println!("\x1b[31m>> [API ERROR] {}\x1b[0m", err);
            return Ok((String::new(), start.elapsed().as_millis()));
        }

        let mut full_response = String::new();
        let mut line_buffer = String::new();
        let mut is_thinking = false;
        let mut stream = response.bytes_stream();

        print!("\n\x1b[38;5;51m[NEXUS STREAM] \x1b[0m");

        while let Some(chunk_result) = stream.next().await {
            if let Ok(chunk) = chunk_result {
                line_buffer.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
                while let Some(pos) = line_buffer.find('\n') {
                    let line = line_buffer[..pos].to_string();
                    line_buffer.drain(..pos + 1);
                    let clean = line.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" { continue; }

                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                            let mut remaining = token;
                            while !remaining.is_empty() {
                                if !is_thinking {
                                    if let Some(start_pos) = remaining.find("<THOUGHT>") {
                                        let clean_part = &remaining[..start_pos];
                                        self.broadcast_token(clean_part, false);
                                        print!("\x1b[38;5;253m{}\x1b[0m", clean_part);
                                        full_response.push_str(clean_part);
                                        is_thinking = true;
                                        print!("\x1b[38;5;240m<THOUGHT>");
                                        full_response.push_str("<THOUGHT>");
                                        remaining = &remaining[start_pos + 9..];
                                    } else {
                                        self.broadcast_token(remaining, false);
                                        print!("\x1b[38;5;253m{}\x1b[0m", remaining);
                                        full_response.push_str(remaining);
                                        remaining = "";
                                    }
                                } else {
                                    if let Some(end_pos) = remaining.find("</THOUGHT>") {
                                        let thought_part = &remaining[..end_pos];
                                        self.broadcast_token(thought_part, true);
                                        print!("{}", thought_part);
                                        full_response.push_str(thought_part);
                                        is_thinking = false;
                                        print!("</THOUGHT>\x1b[0m");
                                        full_response.push_str("</THOUGHT>");
                                        remaining = &remaining[end_pos + 10..];
                                    } else {
                                        self.broadcast_token(remaining, true);
                                        print!("{}", remaining);
                                        full_response.push_str(remaining);
                                        remaining = "";
                                    }
                                }
                            }
                            io::stdout().flush().ok();
                        }
                    }
                }
            }
        }
        println!("\n\x1b[38;5;244m────────────────────────────────\x1b[0m");
        METRICS.record_api_call(start.elapsed().as_millis());
        Ok((full_response, start.elapsed().as_millis()))
    }

    fn broadcast_token(&self, text: &str, is_thinking: bool) {
        if text.is_empty() { return; }
        let msg = json!({
            "type": "NEURAL_STREAM",
            "payload": { "text": text, "is_thinking": is_thinking }
        }).to_string();
        let _ = self.tx.send(msg);
    }
}
