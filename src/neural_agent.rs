use anyhow::Result;
use serde_json::json;
use reqwest::Client;

pub struct MicroAgent;

impl MicroAgent {
    pub fn init() -> Result<Self> {
        // Initialization is instant (0MB RAM usage)
        Ok(MicroAgent)
    }

    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str) -> Result<String> {
        // 1. Safety Check: Ensure API Key is present
        if api_key == "DUMMY" || api_key.is_empty() {
            return Ok(">> [SYSTEM] API Key missing. Neural Link offline.".to_string());
        }

        // 2. TARGET MODEL: gemma-3-27b-it
        // Note: verify this URL version (v1beta) matches your API key permissions
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}",
            api_key
        );

        let payload = json!({
            "contents": [{
                "parts": [{"text": user_text}]
            }]
        });

        // 3. Send Request
        let resp = client.post(&url)
            .json(&payload)
            .send()
            .await?;

        // 4. Handle HTTP Errors (e.g., 400 Bad Request, 403 Forbidden)
        if !resp.status().is_success() {
             return Ok(format!(">> [API ERROR] HTTP Status: {}", resp.status()));
        }

        // 5. Parse JSON Response
        let body: serde_json::Value = resp.json().await?;

        // 6. Deep Extraction of Text (Safe-Guard against crashes)
        if let Some(candidates) = body.get("candidates") {
            if let Some(first) = candidates.get(0) {
                // Happy Path: Text found
                if let Some(content) = first.get("content") {
                     if let Some(parts) = content.get("parts") {
                        if let Some(text_part) = parts.get(0) {
                            if let Some(text) = text_part.get("text") {
                                return Ok(text.as_str().unwrap_or("").to_string());
                            }
                        }
                     }
                }
                // Sad Path: Safety Block or Filter
                if let Some(finish_reason) = first.get("finishReason") {
                    return Ok(format!(">> [GEMMA-3 BLOCKED] Reason: {}", finish_reason));
                }
            }
        }

        Ok(">> [GEMMA-3] No readable text response received.".to_string())
    }
}
