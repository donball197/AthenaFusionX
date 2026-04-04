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
        
        // UPGRADE: Shifting default cortex to Gemma 4 31B
        let model = ctx.target_model.unwrap_or_else(|| "gemma-4-31b-it".to_string());
        
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", 
            model, 
            api_key
        );

        let payload = json!({ 
            "contents": [{ "parts": [{ "text": user_text }] }],
            "generationConfig": {
                "temperature": 0.15,
                "maxOutputTokens": 32768, // LEVERAGING GEMMA 4 CAPACITY
                "topP": 0.95
            }
        });
        
        let response = client.post(url).json(&payload).send().await
            .map_err(|e| NexusError::Other(format!("API_IGNITION_FAILURE: {}", e)))?;

        let mut full_text = String::new();
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            if let Ok(bytes_chunk) = item {
                let chunk_str = String::from_utf8_lossy(&bytes_chunk);
                for line in chunk_str.lines() {
                    if line.starts_with("data: ") {
                        if let Ok(val) = serde_json::from_str::<Value>(&line[6..]) {
                            if let Some(token) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                                full_text.push_str(token);
                                let _ = self.tx.send(json!({"type": "ghost_token", "payload": token}).to_string());
                            }
                        }
                    }
                }
            }
        }

        let duration = start.elapsed().as_millis();
        Ok((full_text, duration))
    }
}
