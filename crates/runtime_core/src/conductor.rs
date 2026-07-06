use crate::factory::AgentFactory;
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

pub struct TurnContext<'a> {
    pub orch: &'a Orchestrator,
    pub client: &'a Client,
    pub api_key: &'a str,
    pub tx: Option<broadcast::Sender<String>>,
}

impl Conductor {
    pub async fn run_turn(
        profile: &AgentProfile,
        prompt: String,
        turn: usize,
        ctx: TurnContext<'_>,
        errors: usize,
    ) -> Result<String, String> {
        METRICS.api_calls.fetch_add(1, Ordering::Relaxed);

        let model = AgentFactory::get_model_for_turn(turn, errors);
        let _ = wait_for_quota(&model, ESTIMATED_TOKENS).await;

        let neural_ctx = ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(model.clone()),
            ..Default::default()
        };

        if let Some(ref sender) = ctx.tx {
            let _ = sender.send(
                json!({"type": "NEURAL_THOUGHT", "payload": format!("[CONDUCTOR] turn {} → {}", turn, model)}).to_string()
            );
        }

        let result = ctx.orch.run(prompt.clone(), neural_ctx, ctx.client, ctx.api_key, ctx.tx.clone()).await;

        if let Err(ref e) = result {
            if e.contains("STALE") || e.contains("500") || e.contains("RESOURCE_EXHAUSTED") || e.contains("INTERNAL") || e.contains("CONTEXT_OVERFLOW") {
                METRICS.task_failures.fetch_add(1, Ordering::Relaxed);
                
                let failover = AgentFactory::failover_model(&model);
                tracing::warn!("[CONDUCTOR] {} failed → failing over to {}", model, failover);
                let _ = wait_for_quota(&failover, ESTIMATED_TOKENS).await;

                let fail_ctx = ContextPacket {
                    system_prompt: Some(profile.persona.clone()),
                    model_override: Some(failover.clone()),
                    ..Default::default()
                };
                return ctx.orch.run(prompt, fail_ctx, ctx.client, ctx.api_key, ctx.tx).await;
            }
        }
        result
    }
}
