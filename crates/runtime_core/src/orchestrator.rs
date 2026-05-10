use crate::neural_agent::ContextPacket;
use crate::agent_protocol::{ModelTier, SubagentReturn};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::broadcast;

pub struct Orchestrator;
impl Orchestrator {
    pub fn new() -> Self { Self }
    pub async fn run(&self, prompt: String, ctx: ContextPacket, client: &Client, api_key: &str, tx: Option<broadcast::Sender<String>>) -> Result<String, String> {
        let model = "models/gemma-4-26b-a4b-it";
        let url = format!("https://generativelanguage.googleapis.com/v1beta/{}:streamGenerateContent?alt=sse&key={}", model, api_key);
        let payload = json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "systemInstruction": {"parts": [{"text": ctx.system_prompt.unwrap_or_default()}]},
            "generationConfig": {"temperature": 0.7, "maxOutputTokens": 4096}
        });
        let res = client.post(&url).json(&payload).send().await.map_err(|e| e.to_string())?;
        let mut full_text = String::new();
        let mut stream = res.bytes_stream();
        let mut buf = Vec::new();
        while let Some(Ok(chunk)) = stream.next().await {
            buf.extend_from_slice(&chunk);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line_bytes = buf.drain(..=pos).collect::<Vec<u8>>();
                let line = String::from_utf8_lossy(&line_bytes);
                let clean = line.trim_start_matches("data: ").trim();
                if clean.is_empty() || clean == "[DONE]" { continue; }
                if let Ok(val) = serde_json::from_str::<Value>(clean) {
                    if let Some(token) = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
                        full_text.push_str(token);
                        if let Some(ref s) = tx { let _ = s.send(json!({"type":"NEURAL_STREAM","payload":token}).to_string()); }
                    }
                }
            }
        }
        Ok(full_text.trim().to_string())
    }

    pub async fn run_subagent(&self, inst: String, tier: ModelTier, limit: usize, key: &str) -> SubagentReturn {
        let model = match tier {
            ModelTier::Matriarch => "models/gemma-4-26b-a4b-it",
            ModelTier::Specialist => "models/gemini-3.1-flash-lite",
        };
        let url = format!("https://generativelanguage.googleapis.com/v1beta/{}:generateContent?key={}", model, key);
        let payload = json!({"contents": [{"role": "user", "parts": [{"text": inst}]}], "generationConfig": {"temperature": 0.2, "maxOutputTokens": limit}});
        
        let _ = nexus_substrate::token_ledger::wait_for_quota(model, limit as u32).await;
        
        let client = Client::new();
        let res = client.post(&url).json(&payload).send().await.unwrap();
        let val: Value = res.json().await.unwrap_or_default();
        let content = val.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()).unwrap_or("");
        let reason = val.pointer("/candidates/0/finishReason").and_then(|v| v.as_str()).unwrap_or("SUCCESS");
        SubagentReturn {
            status: if reason == "MAX_TOKENS" || reason == "LENGTH" { "CONTEXT_EXHAUSTED".into() } else { "SUCCESS".into() },
            output: content.to_string(),
            monologue: None
        }
    }
}
