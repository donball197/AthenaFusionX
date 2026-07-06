// src/conductor.rs — v3.0 Resilient Conductor
use crate::router::AgentProfile;
use crate::orchestrator::Orchestrator;
use crate::neural_agent::ContextPacket;
use reqwest::Client;
use tokio::sync::broadcast;
use serde_json::json;

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
        // Selection: Tier escalation logic
        let model = if turn > 6 { "gemini-2.5-pro" }
                    else if turn > 3 { "gemma-4-31b-it" }
                    else { profile.primary_model };

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

        // ⚡ AUTOMATIC FAILOVER: If model returns 500, retry with stable MoE
        if let Err(ref e) = result {
            if e.contains("500") || e.contains("INTERNAL") {
                if let Some(ref sender) = tx {
                    let _ = sender.send(json!({
                        "type": "NEURAL_THOUGHT",
                        "payload": format!("!! [CONDUCTOR] Model {} unstable. Initiating failover to stable MoE...", model)
                    }).to_string());
                }

                let failover_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some("gemma-4-31b-it".to_string()),
                    ..Default::default()
                };
                return orch.run(prompt, failover_ctx, client, api_key, tx).await;
            }
        }

        result
    }
}
