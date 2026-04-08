use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::orchestrator::Orchestrator;
use crate::nexus_error::NexusError;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RiskReport {
    pub risk_score: f32,
    pub logic_flaws: Vec<String>,
    pub recommendation: String,
    pub can_proceed: bool,
}

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
    pub orchestrator: Option<Arc<Orchestrator>>,
}

impl ArbiterAgent {
    pub fn new(agent: Arc<MicroAgent>, orchestrator: Option<Arc<Orchestrator>>) -> Self {
        Self { agent, orchestrator }
    }

    pub async fn analyze_directive(&self, api_key: &str, directive: &str) -> Result<RiskReport, NexusError> {
        let system_prompt = "You are the NEXUS ARBITER. \
            Analyze risk and output ONLY valid JSON in this exact shape: \
            {\"risk_score\": float 0.0-1.0, \"logic_flaws\": [], \"recommendation\": \"...\", \"can_proceed\": bool}";

        let ctx = ContextPacket {
            system_prompt: Some(system_prompt.to_string()),
            ..Default::default()
        };

        let client = reqwest::Client::new();

        let (response, _) = self.agent.prepare_and_run(&client, api_key, directive, ctx, 0.3)
            .await
            .map_err(|e| NexusError::NeuralLinkSevered(e.to_string()))?;

        // More robust JSON parsing with fallback
        let report: RiskReport = serde_json::from_str(&response)
            .or_else(|_| serde_json::from_str(&format!("{{ \"risk_score\": 0.9, \"logic_flaws\": [\"Parse failed\"], \"recommendation\": \"{}\", \"can_proceed\": false }}", response.trim())))
            .unwrap_or(RiskReport {
                risk_score: 1.0,
                logic_flaws: vec!["JSON Parse Error".into()],
                recommendation: "ABORT: Neural Logic Failure".into(),
                can_proceed: false,
            });

        Ok(report)
    }
}
