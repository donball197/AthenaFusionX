use std::sync::atomic::Ordering;
use tokio::sync::broadcast;
use reqwest::Client;

use crate::metrics::METRICS;
use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::router::IntelligentRouter;

pub struct Orchestrator;

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(
        &self,
        prompt: String,
        ctx: ContextPacket,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        METRICS.api_calls.fetch_add(1, Ordering::SeqCst);
        
        let mut active_ctx = ctx;

        // Only classify and assign defaults if properties are not already defined
        if active_ctx.system_prompt.is_none() || active_ctx.model_override.is_none() {
            let profile = IntelligentRouter::route(&prompt);

            if active_ctx.system_prompt.is_none() {
                active_ctx.system_prompt = Some(profile.persona.clone());
            }
            if active_ctx.model_override.is_none() {
                active_ctx.model_override = Some(profile.primary_model.to_string());
            }
        }

        // 3. Create a scoped agent for the execution turn
        let sender = tx.unwrap_or_else(|| {
            let (dummy_tx, _) = broadcast::channel(1);
            dummy_tx
        });
        
        let agent = MicroAgent::init(sender).map_err(|e| format!("AGENT_INIT_FAULT: {}", e))?;

        // 4. Engage the API and pipe the SSE stream
        match agent.prepare_and_run(client, api_key, &prompt, active_ctx, 0.0, None).await {
            Ok((execution_delta, _latency)) => Ok(execution_delta),
            Err(e) => Err(format!("NEURAL_UPLINK_FAILURE: {}", e)),
        }
    }
}
