// src/arbiter_agent.rs - v3.0 Ensemble Voting Arbiter (Cleaned)
use crate::neural_agent::MicroAgent;
use crate::metrics::METRICS;
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

    async fn probe(&self, client: &reqwest::Client, api_key: &str, directive: &str, temperature: f32) -> RiskReport {
        let system_prompt = format!(
            "You are the NEXUS ARBITER. Temperature setting: {:.1}. \
             Analyze risk and return ONLY valid JSON with fields: \
             risk_score (0.0-1.0), logic_flaws (array of strings), \
             recommendation (string), can_proceed (bool). No markdown.",
            temperature
        );

        let model = std::env::var("NEXUS_MODEL")
            .unwrap_or_else(|_| "gemma-4-26b-a4b-it".to_string());

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );

        let payload = serde_json::json!({
            "system_instruction": { "parts": [{ "text": system_prompt }] },
            "contents": [{ "role": "user", "parts": [{ "text": directive }] }],
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": 512,
                "topP": 0.95
            }
        });

        let fallback = RiskReport {
            risk_score: 0.85,
            logic_flaws: vec!["Probe failed — defaulting to high risk".into()],
            recommendation: "High risk — probe did not complete".into(),
            can_proceed: false,
        };

        let Ok(resp) = client.post(&url).json(&payload).send().await else { return fallback; };
        let Ok(body) = resp.json::<serde_json::Value>().await else { return fallback; };

        let text = body
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let clean = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str(clean).unwrap_or(fallback)
    }

    pub async fn analyze_directive(&self, api_key: &str, directive: &str) -> Result<RiskReport> {
        let client = reqwest::Client::new();

        let (r1, r2, r3) = tokio::join!(
            self.probe(&client, api_key, directive, 0.1),
            self.probe(&client, api_key, directive, 0.3),
            self.probe(&client, api_key, directive, 0.5),
        );

        let avg_risk = (r1.risk_score + r2.risk_score + r3.risk_score) / 3.0;
        let all_proceed = r1.can_proceed && r2.can_proceed && r3.can_proceed;

        let mut all_flaws = r1.logic_flaws.clone();
        for flaw in r2.logic_flaws.iter().chain(r3.logic_flaws.iter()) {
            if !all_flaws.contains(flaw) {
                all_flaws.push(flaw.clone());
            }
        }

        let recommendation = if !all_proceed {
            format!(
                "ENSEMBLE BLOCK — Votes: [{}/{}/{}] | Avg risk: {:.2} | {}",
                r1.can_proceed as u8, r2.can_proceed as u8, r3.can_proceed as u8,
                avg_risk, r1.recommendation
            )
        } else {
            format!(
                "ENSEMBLE PASS — Unanimous | Avg risk: {:.2} | {}",
                avg_risk, r1.recommendation
            )
        };

        if !all_proceed {
            METRICS.arbiter_blocks.inc();
        }

        Ok(RiskReport {
            risk_score: avg_risk,
            logic_flaws: all_flaws,
            recommendation,
            can_proceed: all_proceed,
        })
    }
}
