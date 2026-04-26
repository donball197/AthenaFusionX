// conductor.rs — v3.3
// FIX: Failover to gemma-4-26b-a4b-it now calls wait_for_quota before
//      issuing the retry request. The original skipped the ledger entirely
//      on 429 errors, potentially hammering the same rate-limited endpoint.
// FIX: Escalation threshold stays at turn > 6 (was accidentally lowered).
use crate::neural_agent::ContextPacket;
use crate::orchestrator::Orchestrator;
use crate::router::AgentProfile;
use crate::token_ledger::wait_for_quota;
use reqwest::Client;
use serde_json::json;
use tokio::sync::broadcast;

const FAILOVER_MODEL: &str = "gemma-4-26b-a4b-it";
const ESTIMATED_TOKENS: u32 = 2_000;

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
        let model = if turn > 6 { "gemma-4-31b-it" } else { profile.primary_model };

        // Honour quota before every primary attempt
        wait_for_quota(model, ESTIMATED_TOKENS).await;

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(
                json!({
                    "type": "NEURAL_THOUGHT",
                    "payload": format!("[UPLINK] → {} | turn {}", model, turn)
                })
                .to_string(),
            );
        }

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

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
                                "!! [CONDUCTOR] {} unstable. Quota-checking failover to {}...",
                                model, FAILOVER_MODEL
                            )
                        })
                        .to_string(),
                    );
                }

                // FIX: Check quota for failover model too
                wait_for_quota(FAILOVER_MODEL, ESTIMATED_TOKENS).await;

                let failover_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some(FAILOVER_MODEL.to_string()),
                    ..Default::default()
                };
                return orch.run(prompt, failover_ctx, client, api_key, tx).await;
            }
        }

        result
    }
}
