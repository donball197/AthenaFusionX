//! Micro-Agent API Gateway
//! Executes live HTTP/REST dispatch with multi-provider failover cascades,
//! and supports a real tool-calling loop against sandboxed filesystem tools.

use crate::tools;
use nexus_runtime_core::provider::resolve_provider;
use serde_json::{json, Value};
use std::error::Error;
use std::path::Path;

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
    /// No tool access -- plain single-turn completion. Kept for callers that
    /// don't need filesystem grounding.
    pub fn dispatch_cascade(
        &self,
        request: AgentRequest,
        cascade: &[String],
    ) -> Result<AgentResponse, Box<dyn Error>> {
        self.dispatch_inner(request, cascade, None)
    }

    /// Same as dispatch_cascade, but gives the model real, sandboxed
    /// list_dir / read_file tools scoped to `workspace_root`, and loops
    /// executing tool calls until the model returns a final text answer
    /// (or `max_turns` is exceeded).
    pub fn dispatch_with_tools(
        &self,
        request: AgentRequest,
        cascade: &[String],
        workspace_root: &Path,
    ) -> Result<AgentResponse, Box<dyn Error>> {
        self.dispatch_inner(request, cascade, Some(workspace_root))
    }

    fn dispatch_inner(
        &self,
        request: AgentRequest,
        cascade: &[String],
        workspace_root: Option<&Path>,
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

            let mut messages = vec![json!({
                "role": "system",
                "content": "You are a Nexus Micro-Agent executing on an ARM64 edge node. Be concise, technical, and precise. When you need real information about the filesystem, use the provided tools rather than guessing or inventing paths."
            })];
            messages.push(json!({ "role": "user", "content": request.prompt }));

            const MAX_TURNS: usize = 6;
            let mut turn = 0;
            let mut tokens_total = 0usize;

            loop {
                turn += 1;
                if turn > MAX_TURNS {
                    return Err("Tool-call loop exceeded max turns without a final answer.".into());
                }

                let mut payload = json!({
                    "model": model,
                    "messages": messages,
                    "temperature": request.temperature,
                    "max_tokens": request.max_tokens
                });

                if workspace_root.is_some() {
                    payload["tools"] = tools::tool_schema();
                }

                let req = match resolved.auth {
                    nexus_runtime_core::provider::AuthStyle::Bearer => ureq::post(&resolved.endpoint)
                        .set("Authorization", &format!("Bearer {}", resolved.api_key)),
                    nexus_runtime_core::provider::AuthStyle::GoogleApiKey => {
                        ureq::post(&resolved.endpoint).set("x-goog-api-key", &resolved.api_key)
                    }
                };

                let response = match req.set("Content-Type", "application/json").send_json(payload)
                {
                    Ok(r) => r,
                    Err(e) => {
                        println!("  -> [FAIL] {} unreachable or returned error: {}", model, e);
                        nexus_runtime_core::provider::trip_circuit(model);
                        last_error = Some(Box::new(e));
                        break; // fall through to next model in cascade
                    }
                };

                let response_json: Value = response.into_json()?;
                let msg = &response_json["choices"][0]["message"];

                tokens_total += response_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize;

                let tool_calls = msg["tool_calls"].as_array();

                match (tool_calls, workspace_root) {
                    (Some(calls), Some(root)) if !calls.is_empty() => {
                        // Model wants to use tools. Append its turn, then
                        // execute each call for real and append results.
                        messages.push(msg.clone());

                        for call in calls {
                            let call_id = call["id"].as_str().unwrap_or("").to_string();
                            let fn_name = call["function"]["name"].as_str().unwrap_or("");
                            let args_str = call["function"]["arguments"].as_str().unwrap_or("{}");
                            let args: Value =
                                serde_json::from_str(args_str).unwrap_or_else(|_| json!({}));

                            println!("  -> [TOOL CALL] {}({})", fn_name, args_str);
                            let result = tools::execute_tool(root, fn_name, &args);

                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": call_id,
                                "content": result
                            }));
                        }
                        // Loop again with tool results appended.
                        continue;
                    }
                    _ => {
                        // Final answer.
                        let text = msg["content"]
                            .as_str()
                            .unwrap_or("[ERROR: Failed to parse model response]")
                            .to_string();

                        return Ok(AgentResponse {
                            text,
                            tokens_used: tokens_total,
                            model_used: model.to_string(),
                        });
                    }
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
