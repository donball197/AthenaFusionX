use anyhow::Result;
use serde_json::json;
use reqwest::Client;
use std::time::Instant;
use std::fs;
use std::sync::Mutex;
use std::collections::VecDeque;

pub struct MicroAgent {
    // Thread-safe memory buffer
    memory: Mutex<VecDeque<String>>
}

impl MicroAgent {
    pub fn init() -> Result<Self> {
        Ok(MicroAgent {
            memory: Mutex::new(VecDeque::with_capacity(6))
        })
    }

    // UPDATED SIGNATURE: Now accepts 'code_context'
    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str, code_context: Option<String>) -> Result<(String, u128)> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}",
            api_key
        );

        let system_prompt = fs::read_to_string("system_prompt.txt")
            .unwrap_or_else(|_| "You are NEXUS.".to_string());

        // 1. Build Memory String
        let mut context_string = String::new();
        {
            let history = self.memory.lock().unwrap();
            for msg in history.iter() {
                context_string.push_str(msg);
                context_string.push_str("\n");
            }
        }

        // 2. Format the File Context (The "Eyes")
        let file_sight = if let Some(code) = code_context {
            format!("CURRENT_OPEN_FILE_CONTEXT:\n```rust\n{}\n```\n(Use this code to answer the query below)\n", code)
        } else {
            String::new()
        };

        // 3. Construct Full Prompt
        let full_prompt = format!(
            "SYSTEM_INSTRUCTION:\n{}\n\nCONVERSATION_LOG:\n{}\n{}\nOPERATOR_QUERY:\n{}", 
            system_prompt, context_string, file_sight, user_text
        );

        let payload = json!({
            "contents": [{ "parts": [{"text": full_prompt}] }],
            "generationConfig": { "temperature": 0.3, "maxOutputTokens": 2048 }
        });

        let start = Instant::now();
        let resp = client.post(url).json(&payload).send().await?;
        let duration = start.elapsed().as_millis();

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Ok((format!(">> [GEMMA-3 ERROR] API Refused: {}", error_text), duration));
        }

        let body: serde_json::Value = resp.json().await?;
        let text = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or(">> [GEMMA-3] Void response.");

        // 4. Commit to Memory
        {
            let mut history = self.memory.lock().unwrap();
            history.push_back(format!("OPERATOR: {}", user_text));
            history.push_back(format!("NEXUS: {}", text));
            while history.len() > 6 { history.pop_front(); }
        }

        Ok((text.to_string(), duration))
    }
}
