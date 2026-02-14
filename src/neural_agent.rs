use serde_json::json;
use reqwest::Client;

pub struct MicroAgent;

impl MicroAgent {
    pub fn init() -> anyhow::Result<Self> { Ok(Self) }

    pub async fn evolve_kernel(&self, client: &Client, api_key: &str, source: &str, error: Option<&str>) -> anyhow::Result<String> {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}", api_key);
        
        let prompt = match error {
            None => format!("STRICT ARCHITECT MODE. Optimize this code:\n{}", source),
            Some(err) => format!(
                "STRICT DEBUGGER MODE. Your previous code failed validation.\nERROR:\n{}\n\nFIX the code below. Output ONLY the reasoning and the code:\n{}", 
                err, source
            ),
        };

        let body = json!({ "contents": [{ "parts": [{ "text": prompt }] }] });
        let resp = client.post(url).json(&body).send().await?.json::<serde_json::Value>().await?;
        
        Ok(resp["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string())
    }
}
