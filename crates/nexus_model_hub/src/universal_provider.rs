use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    Google,
    OpenAiCompatible, // Covers NVIDIA NIM, Ollama, vLLM, Groq, Together
}

impl ProviderKind {
    pub fn detect(model_or_provider: &str) -> Self {
        let env_provider = env::var("NEXUS_PROVIDER").unwrap_or_default().to_lowercase();
        if env_provider == "nvidia" || env_provider == "openai" || env_provider == "ollama" {
            return ProviderKind::OpenAiCompatible;
        }

        if model_or_provider.contains('/')
            || model_or_provider.contains("llama")
            || model_or_provider.contains("deepseek")
            || model_or_provider.contains("nvidia")
            || model_or_provider.contains("mistral")
        {
            ProviderKind::OpenAiCompatible
        } else {
            ProviderKind::Google
        }
    }
}

pub struct UniversalProvider;

impl UniversalProvider {
    pub async fn generate(
        client: &Client,
        api_key: &str,
        model: &str,
        system_prompt: Option<&str>,
        user_prompt: &str,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<String> {
        let kind = ProviderKind::detect(model);

        match kind {
            ProviderKind::Google => {
                let base_url = env::var("NEXUS_API_GATEWAY")
                    .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string());
                let url = format!(
                    "{}/v1beta/models/{}:generateContent?key={}",
                    base_url.trim_end_matches('/'),
                    model.replace("models/", ""),
                    api_key.trim()
                );

                let mut contents = Vec::new();
                if let Some(sys) = system_prompt {
                    contents.push(json!({
                        "role": "user",
                        "parts": [{"text": format!("SYSTEM INSTRUCTION: {}", sys)}]
                    }));
                }
                contents.push(json!({
                    "role": "user",
                    "parts": [{"text": user_prompt}]
                }));

                let payload = json!({
                    "contents": contents,
                    "generationConfig": {
                        "temperature": temperature,
                        "maxOutputTokens": max_tokens
                    }
                });

                let res = client.post(&url).json(&payload).send().await?;
                let body: Value = res.json().await.unwrap_or_default();

                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    return Err(anyhow!("Google API Error: {}", err));
                }

                let text = body
                    .pointer("/candidates/0/content/parts/0/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                Ok(text.replace("```json", "").replace("```", "").trim().to_string())
            }

            ProviderKind::OpenAiCompatible => {
                let base_url = env::var("NVIDIA_API_GATEWAY")
                    .or_else(|_| env::var("NEXUS_OPENAI_GATEWAY"))
                    .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());

                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

                let mut messages = Vec::new();
                if let Some(sys) = system_prompt {
                    messages.push(json!({"role": "system", "content": sys}));
                }
                messages.push(json!({"role": "user", "content": user_prompt}));

                let payload = json!({
                    "model": model,
                    "messages": messages,
                    "temperature": temperature,
                    "max_tokens": max_tokens
                });

                let auth_key = env::var("NVIDIA_API_KEY").unwrap_or_else(|_| api_key.to_string());

                let res = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", auth_key.trim()))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?;

                let body: Value = res.json().await.unwrap_or_default();

                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    return Err(anyhow!("OpenAI/NIM API Error: {}", err));
                }

                let text = body
                    .pointer("/choices/0/message/content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                Ok(text.replace("```json", "").replace("```", "").trim().to_string())
            }
        }
    }
}
