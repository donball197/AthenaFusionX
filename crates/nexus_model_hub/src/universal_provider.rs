use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    Google,
    Nvidia,
    OpenAiCompatible, // Covers NVIDIA NIM, Ollama, vLLM, Groq, Together
}

impl ProviderKind {
    pub fn detect(model_or_provider: &str) -> Self {
        let model = model_or_provider.to_ascii_lowercase();

        // Explicit provider selection always wins.
        let env_provider = env::var("NEXUS_PROVIDER").unwrap_or_default().to_ascii_lowercase();

        if env_provider == "nvidia" {
            return ProviderKind::Nvidia;
        }

        if env_provider == "openai"
            || env_provider == "ollama"
            || env_provider == "nexus"
            || env_provider == "local"
        {
            return ProviderKind::OpenAiCompatible;
        }

        // AthenaFusionX local S23 / LiteRT-LM models are served
        // through the existing OpenAI-compatible local gateway.
        if model.contains("e4b") || model.contains("litert") || model == "nexus" {
            return ProviderKind::OpenAiCompatible;
        }

        // Cloud/OpenAI-compatible model families.
        if model.contains('/')
            || model.contains("llama")
            || model.contains("deepseek")
            || model.contains("nvidia")
            || model.contains("mistral")
            || model.contains("nemotron")
            || model.contains("meta/")
        {
            return ProviderKind::OpenAiCompatible;
        }

        ProviderKind::Google
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
                let status = res.status();
                let raw = res.text().await?;

                let body: Value = serde_json::from_str(&raw).map_err(|e| {
                    anyhow!("Google API returned HTTP {} with invalid JSON: {}", status, e)
                })?;

                if !status.is_success() {
                    let message = body
                        .pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown Google API error");
                    return Err(anyhow!("Google API HTTP {}: {}", status, message));
                }

                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    return Err(anyhow!("Google API Error: {}", err));
                }

                let text = body
                    .pointer("/candidates/0/content/parts/0/text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("Google API HTTP {} returned no candidate text", status)
                    })?;

                Ok(text.replace("```json", "").replace("```", "").trim().to_string())
            }

            ProviderKind::Nvidia | ProviderKind::OpenAiCompatible => {
                let base_url = env::var("OPENAI_BASE_URL")
                    .or_else(|_| env::var("NEXUS_OPENAI_GATEWAY"))
                    .or_else(|_| env::var("NEXUS_API_GATEWAY"))
                    .or_else(|_| env::var("NVIDIA_API_GATEWAY"))
                    .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());

                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

                let mut messages = Vec::new();
                if let Some(sys) = system_prompt {
                    messages.push(json!({"role": "system", "content": sys}));
                }
                messages.push(json!({"role": "user", "content": user_prompt}));

                let mut payload = json!({
                    "model": model,
                    "messages": messages,
                    "temperature": temperature,
                    "max_tokens": max_tokens
                });

                if kind == ProviderKind::Nvidia {
                    payload["chat_template_kwargs"] = json!({
                        "enable_thinking": false,
                        "thinking": false,
                        "force_nonempty_content": true
                    });
                }

                if kind == ProviderKind::Nvidia && model.starts_with("z-ai/") {
                    // GLM on NIM ignores chat_template_kwargs; it uses a top-level field.
                    if let Some(o) = payload.as_object_mut() {
                        o.remove("chat_template_kwargs");
                    }
                    payload["reasoning_effort"] = json!("low");
                }

                let auth_key = env::var("OPENAI_API_KEY")
                    .or_else(|_| env::var("NEXUS_AUTH_KEY"))
                    .or_else(|_| env::var("NVIDIA_API_KEY"))
                    .unwrap_or_else(|_| api_key.to_string());

                let res = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", auth_key.trim()))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?;

                let status = res.status();
                let raw = res.text().await?;

                let body: Value = serde_json::from_str(&raw).map_err(|e| {
                    anyhow!(
                        "OpenAI-compatible endpoint returned HTTP {} with invalid JSON: {}",
                        status,
                        e
                    )
                })?;

                if !status.is_success() {
                    let message = body
                        .pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown OpenAI-compatible API error");
                    return Err(anyhow!("OpenAI-compatible API HTTP {}: {}", status, message));
                }

                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    return Err(anyhow!("OpenAI-compatible API Error: {}", err));
                }

                let text = body
                    .pointer("/choices/0/message/content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("OpenAI-compatible API HTTP {} returned no message content", status)
                    })?;

                Ok(text.replace("```json", "").replace("```", "").trim().to_string())
            }
        }
    }
}
