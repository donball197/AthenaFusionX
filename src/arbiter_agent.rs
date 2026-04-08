// src/arbiter_agent.rs - Hardened Risk Analysis
use crate::neural_agent::{MicroAgent, ContextPacket};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RiskReport {
    pub risk_score: f32,
    pub logic_flaws: Vec<String>,
    pub recommendation: String,
    pub can_proceed: bool,
}

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
}

impl ArbiterAgent {
    pub fn new(agent: Arc<MicroAgent>, _orchestrator: Option<Arc<crate::orchestrator::Orchestrator>>) -> Self {
        Self { agent }
    }

    pub async fn analyze_directive(&self, api_key: &str, directive: &str) -> Result<RiskReport> {
        let system_prompt = "You are the NEXUS ARBITER. Analyze risk and return ONLY valid JSON with risk_score (0.0-1.0), logic_flaws, recommendation, and can_proceed.";

        let ctx = ContextPacket {
            system_prompt: Some(system_prompt.to_string()),
            ..Default::default()
        };

        let client = reqwest::Client::new();
        let (response, _) = self.agent.prepare_and_run(&client, api_key, directive, ctx, 0.3).await?;

        // Robust JSON fallback
        let report: RiskReport = serde_json::from_str(&response)
            .unwrap_or_else(|_| RiskReport {
                risk_score: 0.85,
                logic_flaws: vec!["JSON parse failed".into()],
                recommendation: "High risk — proceed with caution".into(),
                can_proceed: false,
            });

        Ok(report)
    }
}
