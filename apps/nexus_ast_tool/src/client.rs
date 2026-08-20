use anyhow::{anyhow, Result};
use serde_json::json;
use std::env;
use std::thread;
use std::time::Duration;

pub struct ApiClient;

impl ApiClient {
    pub fn generate_optimization(symbol: &str, code_snippet: &str) -> anyhow::Result<String> {
        let model = std::env::var("NEXUS_LIGHT_MODEL").unwrap_or_else(|_| "gemini-1.5-flash".to_string());
        let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
        let endpoint = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);

        let prompt = format!(
            "You are the Nexus Singularity Architect. Optimize this Rust function '{}' for better performance, readability, or idiomatic Rust.\n\nSource:\n```rust\n{}\n```\n\nReturn ONLY the optimized Rust statements for the replacement function body wrapped EXACTLY in <payload> and </payload> tags. Do not include outer braces.",
            symbol, code_snippet
        );

        let body = serde_json::json!({ "contents": [{ "parts": [{ "text": prompt }] }] });
        let response = ureq::post(&endpoint).set("Content-Type", "application/json").send_json(body)?;
        
        let json: serde_json::Value = response.into_json()?;
        let text = json["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string();

        let start = text.find("<payload>").ok_or_else(|| anyhow::anyhow!("Missing <payload>"))? + 9;
        let end = text.rfind("</payload>").ok_or_else(|| anyhow::anyhow!("Missing </payload>"))?;
        Ok(text[start..end].replace("```rust\n", "").replace("```", "").trim().to_string())
    }

}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiClient {
    pub fn new() -> Self {
        Self
    }

    pub fn query_model(instruction: &str, symbol: &str, code_snippet: &str, strategy: &str) -> Result<String> {
        Self::query_model_internal(instruction, symbol, code_snippet, strategy, None)
    }

    pub fn query_model_retry(instruction: &str, symbol: &str, code_snippet: &str, strategy: &str, validation_error: &str, previous_payload: &str) -> Result<String> {
        Self::query_model_internal(instruction, symbol, code_snippet, strategy, Some((validation_error, previous_payload)))
    }

    
    pub fn triage_vulnerability_batch(findings_json: &str) -> Result<String> {
        // Anti-429 Throttle
        thread::sleep(Duration::from_millis(2000));

        let model = env::var("NEXUS_LIGHT_MODEL").or_else(|_| env::var("NEXUS_MODEL")).unwrap_or_else(|_| "gemini-3.5-flash".to_string());
        let is_gemini = model.to_lowercase().contains("gemini");

        let api_key = if is_gemini {
            env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY required"))?
        } else {
            env::var("NVIDIA_API_KEY").map_err(|_| anyhow!("NVIDIA_API_KEY required"))?
        };

        let endpoint = if is_gemini {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key)
        } else {
            "https://integrate.api.nvidia.com/v1/chat/completions".to_string()
        };

        let prompt = format!(
            r#"You are an elite Security Triage AI. Analyze the following JSON array of code snippets for vulnerabilities.
For each snippet, determine if it is exploitable based on the provided context.
Input JSON Array:
```json
{}

```
Return ONLY a JSON array of results wrapped in <payload> tags.
The array must match the input length and format:
[{{"id": 0, "is_exploitable": true, "severity": "High", "reason": "..."}}]"#,
            findings_json
        );

        let body = if is_gemini { json!({ "contents": [{ "parts": [{ "text": prompt }] }] }) }
        else { json!({ "model": model, "messages": [{ "role": "user", "content": prompt }], "temperature": 0.1 }) };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");

        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }

        let response = req.send_json(body).map_err(|e| anyhow!("HTTP req failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let start = text.find("<payload>").ok_or_else(|| anyhow!("Missing <payload>"))? + 9;
        let end = text.rfind("</payload>").ok_or_else(|| anyhow!("Missing </payload>"))?;

        // ✅ Fixed: actual newline, not literal backslash-n
        Ok(text[start..end].replace("```json\n", "").replace("```", "").trim().to_string())
    }
    pub fn triage_vulnerability(finding_type: &str, code_snippet: &str) -> Result<(bool, String, String)> {
        // Anti-429 Throttle
        thread::sleep(Duration::from_millis(2000));
        
        let model = env::var("NEXUS_LIGHT_MODEL").or_else(|_| env::var("NEXUS_MODEL")).unwrap_or_else(|_| "gemini-3.5-flash".to_string());
        let is_gemini = model.to_lowercase().contains("gemini");
        
        let api_key = if is_gemini {
            env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY required"))?
        } else {
            env::var("NVIDIA_API_KEY").map_err(|_| anyhow!("NVIDIA_API_KEY required"))?
        };

        let endpoint = if is_gemini {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key)
        } else {
            "https://integrate.api.nvidia.com/v1/chat/completions".to_string()
        };

        let prompt = format!(
            "You are an elite Security Triage AI.\nAnalyze this code for a '{}' vulnerability.\nSource:\n```\n{}\n```\n\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\n{{\"is_exploitable\": true, \"severity\": \"High\", \"reason\": \"...\"}}",
            finding_type, code_snippet
        );

        let body = if is_gemini { json!({ "contents": [{ "parts": [{ "text": prompt }] }] }) } 
        else { json!({ "model": model, "messages": [{ "role": "user", "content": prompt }], "temperature": 0.1 }) };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");
        
        // Dynamically inject NVIDIA Auth Header
        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }
        
        let response = req.send_json(body).map_err(|e| anyhow!("HTTP req failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let start = text.find("<payload>").ok_or_else(|| anyhow!("Missing <payload>"))? + 9;
        let end = text.rfind("</payload>").ok_or_else(|| anyhow!("Missing </payload>"))?;
        let json_str = text[start..end].replace("```json\n", "").replace("```", "").trim().to_string();

        let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| anyhow!("JSON Parse Error: {}", e))?;
        Ok((parsed["is_exploitable"].as_bool().unwrap_or(false), parsed["severity"].as_str().unwrap_or("Unknown").to_string(), parsed["reason"].as_str().unwrap_or("None").to_string()))
    }

    pub fn generate_security_patch(finding_type: &str, code_snippet: &str, lang: &str) -> Result<String> {
        // Anti-429 Throttle
        thread::sleep(Duration::from_millis(2000));

        let model = env::var("NEXUS_LIGHT_MODEL").or_else(|_| env::var("NEXUS_MODEL")).unwrap_or_else(|_| "gemini-3.5-flash".to_string());
        let is_gemini = model.to_lowercase().contains("gemini");
        
        let api_key = if is_gemini {
            env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY required"))?
        } else {
            env::var("NVIDIA_API_KEY").map_err(|_| anyhow!("NVIDIA_API_KEY required"))?
        };

        let endpoint = if is_gemini {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key)
        } else {
            "https://integrate.api.nvidia.com/v1/chat/completions".to_string()
        };

        let prompt = format!(
            "You are an elite Security Auto-Healer AI.\nFix this '{}' vulnerability in this {} code:\n```\n{}\n```\n\
             CRITICAL: Provide the exact, secure replacement code. Keep the exact same indentation.\n\
             Do NOT include markdown fences. Wrap your code EXACTLY in <payload> and </payload> tags.",
            finding_type, lang, code_snippet
        );

        let body = if is_gemini { json!({ "contents": [{ "parts": [{ "text": prompt }] }] }) } 
        else { json!({ "model": model, "messages": [{ "role": "user", "content": prompt }], "temperature": 0.1 }) };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");
        
        // Dynamically inject NVIDIA Auth Header
        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }

        let response = req.send_json(body).map_err(|e| anyhow!("HTTP req failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let start = text.find("<payload>").ok_or_else(|| anyhow!("Missing <payload>"))? + 9;
        let end = text.rfind("</payload>").ok_or_else(|| anyhow!("Missing </payload>"))?;
        Ok(text[start..end].replace("```python\n", "").replace("```php\n", "").replace("```", "").trim_end().to_string())
    }

    fn query_model_internal(_instruction: &str, _symbol: &str, _code_snippet: &str, _strategy: &str, _correction: Option<(&str, &str)>) -> Result<String> {
        Ok("".to_string()) 
    }

    fn extract_text(response: ureq::Response, is_gemini: bool) -> Result<String> {
        let json: serde_json::Value = response.into_json()?;
        if is_gemini {
            Ok(json["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string())
        } else {
            Ok(json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        }
    }
}
