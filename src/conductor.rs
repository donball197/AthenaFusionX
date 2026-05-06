use crate::metrics::METRICS;
use crate::neural_agent::ContextPacket;
use crate::orchestrator::Orchestrator;
use crate::router::AgentProfile;
use nexus_substrate::token_ledger::wait_for_quota;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;

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
        METRICS.api_calls.fetch_add(1, Ordering::Relaxed);

        let model: &str = if turn > 6 { "gemma-4-26b-a4b-it" } else { "gemma-4-26b-a4b-it" };
        wait_for_quota(model, ESTIMATED_TOKENS).await;

        let ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(
                json!({"type": "NEURAL_THOUGHT", "payload": format!("[CONDUCTOR] turn {} → {}", turn, model)}).to_string()
            );
        }

        let result = orch.run(prompt.clone(), ctx, client, api_key, tx.clone()).await;

        if let Err(ref e) = result {
            if e.contains("STALE") || e.contains("500") || e.contains("RESOURCE_EXHAUSTED") || e.contains("INTERNAL") || e.contains("CONTEXT_OVERFLOW") {
                METRICS.task_failures.fetch_add(1, Ordering::Relaxed);
                
                let failover: &str = if model == "gemma-4-26b-a4b-it" { "gemma-4-26b-a4b-it" } else { "gemma-4-26b-a4b-it" };
                wait_for_quota(failover, ESTIMATED_TOKENS).await;

                let fail_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some(failover.to_string()),
                    ..Default::default()
                };
                return orch.run(prompt, fail_ctx, client, api_key, tx).await;
            }
        }
        result
    }
}
