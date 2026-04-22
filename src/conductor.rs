// src/conductor.rs — v3.2 Resilient Conductor
// FIX: Model escalation was triggering at turn>3, causing all 5 remaining
//      turns to use gemma-4-31b-it (2 RPM limit) and exhaust quota.
//      Now escalates only at turn>6 to preserve free-tier budget.
// FIX: 429/RESOURCE_EXHAUSTED triggers failover, not just 500.
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
        // FIX: Only escalate to 31b on turn > 6 (was > 3).
        // This keeps turns 1-6 on the 15 RPM MoE model,
        // preserving budget for the more critical late turns.
        let model = if turn > 6 { "gemma-4-31b-it" } else { profile.primary_model };

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(
                json!({
                    "type": "NEURAL_THOUGHT",
                    "payload": format!("[UPLINK] → {} | 256K ctx | streaming SSE", model)
                })
                .to_string(),
            );
        }

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

        // Automatic failover on 500, 429, or RESOURCE_EXHAUSTED
        if let Err(ref e) = result {
            if e.contains("500")
                || e.contains("INTERNAL")
                || e.contains("429")
                || e.contains("RESOURCE_EXHAUSTED")
            {
                if let Some(ref sender) = tx {
                    let _ = sender.send(
                        json!({
                            "type": "NEURAL_THOUGHT",
                            "payload": format!(
                                "!! [CONDUCTOR] Model {} unstable/rate-limited. \
                                 Failover to gemma-4-26b-a4b-it...",
                                model
                            )
                        })
                        .to_string(),
                    );
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
