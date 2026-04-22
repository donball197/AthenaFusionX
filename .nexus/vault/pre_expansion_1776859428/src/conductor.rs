// src/conductor.rs — v3.1 Resilient Conductor
use crate::neural_agent::ContextPacket;
use crate::orchestrator::Orchestrator;
use crate::router::AgentProfile;
use reqwest::Client;
use serde_json::json;
use tokio::sync::broadcast;

pub struct Conductor;

impl Conductor {
    pub async fn run_turn(
        profile: &AgentProfile,
        prompt: String,
        turn: usize,
        orch: &Orchestrator,
        client: &Client,
        api_key: &str,
        tx: Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        // Selection: Tier escalation logic capped at free-tier API limits
        let model = if turn > 3 { "gemma-4-31b-it" } else { profile.primary_model };

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

        // ⚡ AUTOMATIC FAILOVER: Catch 500s and 429 Quota Exhaustion, retry with stable MoE
        if let Err(ref e) = result {
            if e.contains("500")
                || e.contains("INTERNAL")
                || e.contains("429")
                || e.contains("RESOURCE_EXHAUSTED")
            {
                if let Some(ref sender) = tx {
                    let _ = sender.send(json!({
                        "type": "NEURAL_THOUGHT",
                        "payload": format!("!! [CONDUCTOR] Model {} unstable or rate-limited. Initiating failover to stable MoE...", model)
                    }).to_string());
                }

                let failover_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some("gemma-4-26b-a4b-it".to_string()),
                    ..Default::default()
                };
                return orch.run(prompt, failover_ctx, client, api_key, tx).await;
            }
        }

        result
    }
}
