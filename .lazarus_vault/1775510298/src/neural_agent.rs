use crate::nexus_error::NexusError;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use reqwest::Client;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::broadcast;
use futures_util::StreamExt;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContextPacket {
    pub current_file: String,
    pub recent_logs: Vec<String>,
    pub file_content: Option<String>,
    pub terminal_log: Option<String>,
    pub system_prompt: Option<String>,
    pub target_model: Option<String>,
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn init(tx: broadcast::Sender<String>) -> Result<Self, NexusError> {
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(16)), tx })
    }

    pub async fn prepare_and_run(
        &self,
        client: &Client,
        api_key: &str,
        user_text: &str,
        ctx: ContextPacket,
        _resonance: f32,
    ) -> Result<(String, u128), NexusError> {
        let start = std::time::Instant::now();
        
        // UPGRADE: Targeting the MoE variant for high-speed, 500-error-free execution
        let model = ctx.target_model.unwrap_or_else(|| "gemma-4-31b-it".to_string());
        
        // GEMMA 4 PROTOCOL: <|think|> trigger in the native system prompt
        let system_instruction = format!(
            "<|think|>SYSTEM_PROMPT: {}\nACTIVE_FILE: {}\nFILE_CONTENT:\n```\n{}\n```",
            ctx.system_prompt.unwrap_or_else(|| "You are NEXUS Ghost, a Sovereign AI Architect.".to_string()),
            ctx.current_file,
            ctx.file_content.unwrap_or_else(|| "None".to_string())
        );

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", 
            model, 
            api_key
        );

        // GEMMA 4 PROTOCOL: Strict Role Separation and Sampling lock
        let payload = json!({ 
            "contents": [
                {
                    "role": "system",
                    "parts": [{ "text": system_instruction }]
                },
                {
                    "role": "user",
                    "parts": [{ "text": format!("DIRECTIVE: {}", user_text) }]
                }
            ],
            "generationConfig": {
                "temperature": 1.0,
                "maxOutputTokens": 8192,
                "topP": 0.95,
                "topK": 64
            }
        });
        
        let _ = self.tx.send(json!({"type": "ghost_start"}).to_string());

        let response = client.post(&url).json(&payload).send().await
            .map_err(|e| NexusError::Other(format!("UPLINK_FAIL: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            let _ = self.tx.send(json!({"type": "ghost_end"}).to_string());
            let formatted_err = format!("API_HTTP_{}: {}", status, err_text);
            let _ = self.tx.send(json!({"type": "error", "payload": &formatted_err}).to_string());
            return Err(NexusError::Other(formatted_err));
        }

        let mut full_text = String::new();
        let mut line_buffer = String::new();
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            if let Ok(bytes) = item {
                // CRLF normalization ensures the parser doesn't fracture
                line_buffer.push_str(&String::from_utf8_lossy(&bytes).replace("\r\n", "\n"));

                while let Some(newline_pos) = line_buffer.find("\n\n") {
                    let chunk = line_buffer[..newline_pos].trim().to_string();
                    line_buffer.drain(..newline_pos + 2);

                    let clean = chunk.trim_start_matches("data: ").trim();
                    if clean.is_empty() || clean == "[DONE]" || clean == "[" || clean == "]" { continue; }
                    
                    if let Ok(val) = serde_json::from_str::<Value>(clean) {
                        if let Some(err) = val.get("error") {
                            let msg = err["message"].as_str().unwrap_or("API error");
                            let _ = self.tx.send(json!({"type": "error", "payload": msg}).to_string());
                            let _ = self.tx.send(json!({"type": "ghost_end"}).to_string());
                            return Err(NexusError::Other(format!("API_STREAM_ERROR: {}", msg)));
                        }
                        if let Some(token) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            full_text.push_str(token);
                            let _ = self.tx.send(json!({"type": "ghost_token", "payload": token}).to_string());
                        }
                    }
                }
            }
        }

        let leftover = line_buffer.trim_start_matches("data: ").trim();
        if !leftover.is_empty() && leftover != "[DONE]" && leftover != "[" && leftover != "]" {
            if let Ok(val) = serde_json::from_str::<Value>(leftover) {
                if let Some(token) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                    full_text.push_str(token);
                    let _ = self.tx.send(json!({"type": "ghost_token", "payload": token}).to_string());
                }
            }
        }

        if full_text.is_empty() {
            let _ = self.tx.send(json!({"type": "error", "payload": "GHOST_CORTEX: Empty response."}).to_string());
        }

        let _ = self.tx.send(json!({"type": "ghost_end"}).to_string());
        Ok((full_text, start.elapsed().as_millis()))
    }
}
