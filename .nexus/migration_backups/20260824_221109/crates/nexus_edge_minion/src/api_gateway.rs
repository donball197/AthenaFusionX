//! Micro-Agent API Gateway
//! Executes live HTTP/REST dispatch with multi-provider failover cascades.

use serde_json::json;
use std::env;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct AgentRequest {
    pub prompt: String,
    pub temperature: f32,
    pub max_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub text: String,
    pub tokens_used: usize,
    pub model_used: String,
}

pub struct ApiGateway;

impl ApiGateway {
    pub fn new() -> Self {
        Self {}
    }

    /// Resolve provider from the model identifier.
    ///
    /// IMPORTANT:
    /// Model family != provider.
    ///
    /// NVIDIA-hosted models must go to NVIDIA's OpenAI-compatible API.
    /// Gemini models go to Google's OpenAI-compatible API.
    /// Unknown models use the configured Nexus endpoint.
    fn resolve_provider(&self, model_name: &str) -> (String, String) {
        let model = model_name.to_ascii_lowercase();

        // Google Gemini
        if model.contains("gemini") {
            return (
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                    .to_string(),
                env::var("GEMINI_API_KEY").unwrap_or_default(),
            );
        }

        // NVIDIA NIM / hosted NVIDIA models.
        //
        // Do NOT send Nemotron through Google's endpoint.
        if model.contains("nvidia")
            || model.contains("nemotron")
        {
            return (
                "https://integrate.api.nvidia.com/v1/chat/completions"
                    .to_string(),
                env::var("NVIDIA_API_KEY").unwrap_or_default(),
            );
        }

        // Everything else is handled by the configured Nexus provider.
        //
        // NOTE:
        // "llama" and "gemma" are model families, not providers.
        // Their actual provider must therefore be determined by the
        // configured endpoint rather than guessed from the model name.
        (
            env::var("NEXUS_API_ENDPOINT")
                .unwrap_or_else(|_| {
                    "https://api.example.com/v1/chat/completions".to_string()
                }),
            env::var("NEXUS_AUTH_KEY").unwrap_or_default(),
        )
    }

    /// Executes the request, cascading through the provided model list on failure.
    pub fn dispatch_cascade(
        &self,
        request: AgentRequest,
        cascade: &[String],
    ) -> Result<AgentResponse, Box<dyn Error>> {
        if cascade.is_empty() {
            return Err("FATAL: Empty cascade list provided.".into());
        }

        let mut last_error: Option<Box<dyn Error>> = None;

        for model in cascade {
            println!("[Gateway] Attempting dispatch to: {}", model);
            let (endpoint, api_key) = self.resolve_provider(model);

            if api_key.is_empty() {
                println!("  -> [WARN] Skipping {} (Missing API Key)", model);
                continue;
            }

            let payload = json!({
                "model": model,
                "messages": [
                    {
                        "role": "system",
                        "content": "You are a Nexus Micro-Agent executing on an ARM64 edge node. Be concise, technical, and precise."
                    },
                    {
                        "role": "user",
                        "content": request.prompt
                    }
                ],
                "temperature": request.temperature,
                "max_tokens": request.max_tokens
            });

            match ureq::post(&endpoint)
                .set("Authorization", &format!("Bearer {}", api_key))
                .set("Content-Type", "application/json")
                .send_json(payload)
            {
                Ok(response) => {
                    let response_json: serde_json::Value = response.into_json()?;

                    let text = response_json["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("[ERROR: Failed to parse model response]")
                        .to_string();

                    let tokens_used =
                        response_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize;

                    return Ok(AgentResponse { text, tokens_used, model_used: model.to_string() });
                }
                Err(e) => {
                    println!("  -> [FAIL] {} unreachable or returned error: {}", model, e);
                    last_error = Some(Box::new(e));
                    // Loop continues to the next model in the cascade
                }
            }
        }

        Err(format!("All models in cascade failed. Last error: {:?}", last_error).into())
    }
}

impl Default for ApiGateway {
    fn default() -> Self {
        Self::new()
    }
}
