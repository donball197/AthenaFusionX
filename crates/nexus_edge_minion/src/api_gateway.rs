//! Micro-Agent API Gateway
//! Executes live HTTP/REST dispatch with multi-provider failover cascades.

use nexus_runtime_core::provider::resolve_provider;
use serde_json::json;
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
            let resolved = resolve_provider(model);

            if resolved.api_key.is_empty() {
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

            let req = match resolved.auth {
                nexus_runtime_core::provider::AuthStyle::Bearer => {
                    ureq::post(&resolved.endpoint)
                        .set("Authorization", &format!("Bearer {}", resolved.api_key))
                }
                nexus_runtime_core::provider::AuthStyle::GoogleApiKey => {
                    ureq::post(&resolved.endpoint)
                        .set("x-goog-api-key", &resolved.api_key)
                }
            };

            match req.set("Content-Type", "application/json").send_json(payload) {
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
                    nexus_runtime_core::provider::trip_circuit(model);
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
