use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::broadcast;

const MAX_HISTORY_TURNS: usize = 12;
const SOFT_TOKEN_LIMIT: usize = 110_000;
const HARD_TOKEN_LIMIT: usize = 140_000;
const RESERVED_TOKENS: usize = 8192;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct ContextPacket {
    pub system_prompt: Option<String>,
    pub model_override: Option<String>,
    pub file_content: Option<String>,
    pub terminal_log: Option<String>,
    pub memory_recall: Option<String>,
    pub system_stats: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("Missing mandatory field: {0}")]
    MissingField(String),
}

#[derive(Default)]
pub struct MicroAgentBuilder {
    system_prompt: Option<String>,
    model: Option<String>,
}

impl MicroAgentBuilder {
    pub fn new() -> Self {
        Self { system_prompt: None, model: None }
    }

    pub fn system_prompt(mut self, prompt: &str) -> Self {
        self.system_prompt = Some(prompt.to_string());
        self
    }

    pub fn model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn build(self, tx: broadcast::Sender<String>) -> Result<MicroAgent, BuilderError> {
        if self.system_prompt.is_none() {
            return Err(BuilderError::MissingField("system_prompt".to_string()));
        }
        if self.model.is_none() {
            return Err(BuilderError::MissingField("model".to_string()));
        }

        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(MAX_HISTORY_TURNS * 2)), tx })
    }
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn builder() -> MicroAgentBuilder {
        MicroAgentBuilder::new()
    }

    pub fn init(tx: broadcast::Sender<String>) -> Result<Self> {
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(MAX_HISTORY_TURNS * 2)), tx })
    }

    pub async fn prepare_and_run(
        &self,
        client: &Client,
        api_key: &str,
        user_text: &str,
        context: ContextPacket,
    ) -> Result<(String, u128)> {
        let start = Instant::now();
        let default_model = std::env::var("NEXUS_HEAVY_MODEL")
            .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string());
        let model = context.model_override.as_deref().unwrap_or(&default_model);

        // 1. Build the enriched text first
        let system_instruction =
            context.system_prompt.clone().unwrap_or_else(|| "You are NEXUS.".to_string());

        let mut enriched =
            format!("SYSTEM: {}\n\nUSER REQUEST:\n{}", system_instruction, user_text);
        if let Some(code) = &context.file_content {
            enriched += &format!("\n\nACTIVE FILE CONTENT:\n```\n{}\n```", code);
        }
        if let Some(logs) = &context.terminal_log {
            enriched += &format!("\n\nRECENT TERMINAL LOGS:\n```\n{}\n```", logs);
        }

        // 2. Setup and trim history with the new text
        let mut history: Vec<Value> = self.memory.lock().unwrap().iter().cloned().collect();
        trim_history_token_aware(&mut history, &enriched);

        history.push(json!({
            "role": "user",
            "parts": [{"text": enriched}]
        }));

        // 3. Now branch on API Provider with `history` fully available
        let is_gemini = model.contains("gemini") || model.contains("gemma");
        let (url, req_payload) = if is_gemini {
            let u = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
                model,
                api_key.trim()
            );
            let p = json!({
                "contents": history,
                "generationConfig": { "maxOutputTokens": 8192, "temperature": 0.15 }
            });
            (u, p)
        } else {
            let u = "https://integrate.api.nvidia.com/v1/chat/completions".to_string();

            let mut messages = Vec::new();
            if let Some(sys) = &context.system_prompt {
                messages.push(json!({"role": "system", "content": sys}));
            }
            for msg in &history {
                if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                    if let Some(text) = msg.pointer("/parts/0/text").and_then(|t| t.as_str()) {
                        let mapped_role = if role == "model" { "assistant" } else { "user" };
                        messages.push(json!({"role": mapped_role, "content": text}));
                    }
                }
            }

            let p = json!({
                "model": model,
                "messages": messages,
                "temperature": 0.15,
                "max_tokens": 8192,
                "stream": true
            });
            (u, p)
        };

        let response =
            client.post(&url).json(&req_payload).send().await.context("API request failed")?;

        let mut full_response = String::new();
        let mut byte_buffer = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            if let Ok(chunk) = chunk_result {
                byte_buffer.extend_from_slice(&chunk);
                while let Some(pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = byte_buffer.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let clean = line.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" {
                        continue;
                    }

                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(parts) =
                            val.pointer("/candidates/0/content/parts").and_then(|v| v.as_array())
                        {
                            for part in parts {
                                if part.get("thought").and_then(|t| t.as_bool()).unwrap_or(false) {
                                    if let Some(token) = part.get("text").and_then(|t| t.as_str()) {
                                        let _ = self.tx.send(
                                            json!({"type": "NEURAL_THOUGHT", "payload": token})
                                                .to_string(),
                                        );
                                    }
                                    continue;
                                }

                                if let Some(token) = part.get("text").and_then(|t| t.as_str()) {
                                    full_response.push_str(token);
                                    let _ = self.tx.send(
                                        json!({"type": "NEURAL_STREAM", "payload": token})
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        {
            let mut mem = self.memory.lock().unwrap();
            mem.push_back(json!({"role": "user", "parts": [{"text": user_text}]}));
            if !full_response.is_empty() {
                mem.push_back(json!({"role": "model", "parts": [{"text": &full_response}]}));
            }
            while mem.len() > MAX_HISTORY_TURNS * 2 {
                mem.pop_front();
            }
        }

        Ok((full_response, start.elapsed().as_millis()))
    }
}

fn rough_token_count(text: &str) -> usize {
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    let is_code_heavy = text.contains("```") || text.contains("{") || text.contains("fn ");
    let factor = if is_code_heavy { 3.6 } else { 4.1 };
    ((chars as f64 / factor) + (words as f64 * 0.3)).ceil() as usize
}

fn estimate_message_tokens(msg: &Value) -> usize {
    let empty_vec = Vec::new();
    let parts = msg.pointer("/parts").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
    parts
        .iter()
        .filter_map(|p| {
            p.get("text")
                .and_then(|t| t.as_str())
                .or_else(|| p.get("functionCall").and_then(|fc| fc.as_str()))
                .or_else(|| p.get("functionResponse").and_then(|fr| fr.as_str()))
        })
        .map(rough_token_count)
        .sum::<usize>()
        + 10
}

fn trim_history_token_aware(history: &mut Vec<Value>, current_user_msg: &str) {
    if history.is_empty() {
        return;
    }

    let current_tokens = rough_token_count(current_user_msg) + RESERVED_TOKENS;
    let mut total_tokens: usize = history.iter().map(estimate_message_tokens).sum();
    total_tokens += current_tokens;

    if total_tokens <= SOFT_TOKEN_LIMIT {
        return;
    }

    while total_tokens > SOFT_TOKEN_LIMIT && history.len() >= 4 {
        if let (Some(old_user), Some(old_model)) = (history.first(), history.get(1)) {
            let removed = estimate_message_tokens(old_user) + estimate_message_tokens(old_model);
            total_tokens = total_tokens.saturating_sub(removed);
        }
        history.drain(0..2);
    }

    while total_tokens > HARD_TOKEN_LIMIT && !history.is_empty() {
        let removed = estimate_message_tokens(&history[0]);
        total_tokens = total_tokens.saturating_sub(removed);
        history.remove(0);
    }
}
