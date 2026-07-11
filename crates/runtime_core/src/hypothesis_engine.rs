use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FaultHypothesis {
    pub description: String,
    pub probability_score: f32,
    pub proposed_fix_action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoTResolution {
    pub root_cause_analysis: String,
    pub hypotheses: Vec<FaultHypothesis>,
    pub selected_path: FaultHypothesis,
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
            "You are the NEXUS Graph of Thoughts (GoT) Engine.\n             A task has failed. You must perform non-linear reasoning:\n             1. Analyze the fault.\n             2. Generate exactly 3 distinct hypotheses for WHY it failed.\n             3. Assign a mathematical probability score (0.0 to 1.0) to each.\n             4. Select the highest probability hypothesis as the 'selected_path'.\n             Output ONLY valid JSON matching this schema:\n             { \"root_cause_analysis\": \"...\", \"hypotheses\": [ { \"description\": \"...\", \"probability_score\": 0.9, \"proposed_fix_action\": \"...\" } ], \"selected_path\": { ... } }";

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

        let base_url = std::env::var("NEXUS_API_GATEWAY")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string());

        let url =
            format!("{}/v1beta/models/gemma-4-31b-it:generateContent?key={}", base_url, api_key);

        let res = client.post(&url).json(&payload).send().await?;
        let body: serde_json::Value = res.json().await?;

        if let Some(err) = body.pointer("/error/message") {
            return Err(anyhow!("GoT API Error: {}", err));
        }

        let text = body
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("{}")
            .replace("```json", "")
            .replace("```", "");

        let resolution: GoTResolution =
            serde_json::from_str(text.trim()).map_err(|e| anyhow!("GoT Schema Fracture: {}", e))?;

        Ok(resolution)
    }
}
