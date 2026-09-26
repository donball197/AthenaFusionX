use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Hypothesis {
    pub description: String,
    pub probability_score: f32,
    pub proposed_fix_action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoTResolution {
    pub root_cause_analysis: String,
    pub hypotheses: Vec<Hypothesis>,
    pub selected_path: Hypothesis,
}

pub struct HypothesisEngine;

impl HypothesisEngine {
    pub async fn evaluate_fault(
        client: &Client,
        api_key: &str,
        objective: &str,
        fault_log: &str,
    ) -> Result<GoTResolution> {
        let system_instruction =
            "You are the NEXUS Graph of Thoughts (GoT) Engine.\n\
             A task has failed. You must perform non-linear reasoning:\n\
             1. Analyze the fault.\n\
             2. Generate exactly 3 distinct hypotheses for WHY it failed.\n\
             3. Assign a mathematical probability score (0.0 to 1.0) to each.\n\
             4. Select the highest probability hypothesis as the 'selected_path'.\n\
             Output ONLY valid JSON matching this schema:\n\
             { \"root_cause_analysis\": \"...\", \"hypotheses\": [ { \"description\": \"...\", \"probability_score\": 0.9, \"proposed_fix_action\": \"...\" } ], \"selected_path\": { ... } }";

        let payload = serde_json::json!({
            "systemInstruction": { "parts": [{ "text": system_instruction }] },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": format!("OBJECTIVE:\n{}\n\nFAULT TRACE:\n{}", objective, fault_log) }]
                }
            ],
            "generationConfig": {
                "temperature": 0.3
            }
        });

        let models = match nexus_algorithms::Router::select("reasoning") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };

        if models.is_empty() {
            return Err(anyhow!(
                "No heavy models configured in NEXUS_HEAVY_MODEL or NEXUS_HEAVY_CASCADE"
            ));
        }

        let text = crate::epistemic_engine::EpistemicEngine::safe_api_call(
            client, api_key, &models, &payload,
        )
        .await?;

        let resolution: GoTResolution =
            serde_json::from_str(text.trim()).map_err(|e| anyhow!("GoT Schema Fracture: {}", e))?;

        Ok(resolution)
    }
}
