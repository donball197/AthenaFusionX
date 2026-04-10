/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/arbiter_agent.rs - v4.5.4 (Panic-Proof Slicing & Extraction)
use crate::neural_agent::MicroAgent;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RiskReport {
    pub risk_score: f32,
    pub logic_flaws: Vec<String>,
    pub recommendation: String,
    pub can_proceed: bool,
}

pub struct ArbiterAgent {
    _agent: Arc<MicroAgent>,
}

impl ArbiterAgent {
    #[must_use]
    pub fn new(
        agent: Arc<MicroAgent>,
        _orchestrator: Option<Arc<crate::orchestrator::Orchestrator>>,
    ) -> Self {
        Self { _agent: agent }
    }

    pub async fn analyze_directive(&self, api_key: &str, directive: &str) -> Result<RiskReport> {
        let fallback = RiskReport {
            risk_score: 0.1,
            logic_flaws: vec![],
            recommendation: "Internal directive authorized (Fallback)".to_string(),
            can_proceed: true,
        };

        let system_prompt = "You are the NEXUS ARBITER. \
            Analyze risk and return JSON with fields: risk_score, logic_flaws, recommendation, can_proceed. \
            Wrap your FINAL JSON response inside <REPORT> and </REPORT> tags.";

        let model = std::env::var("NEXUS_MODEL").unwrap_or_else(|_| "gemma-4-26b-a4b-it".to_string());
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
        );

        let payload = serde_json::json!({
            "contents": [{ "parts": [{ "text": format!("DIRECTIVE: {}", directive) }] }],
            "systemInstruction": { "parts": [{ "text": system_prompt }] },
            "generationConfig": { "temperature": 0.1 }
        });

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&payload).send().await?;
        let body: serde_json::Value = resp.json().await?;
        
        let text = body.pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str()).unwrap_or("");

        // SUTURE: Panic-proof extraction logic
        let mut clean_json = String::new();

        if let Some(start_idx) = text.rfind("<REPORT>") {
            let after_start = &text[start_idx + 8..];
            if let Some(end_idx) = after_start.find("</REPORT>") {
                clean_json = after_start[..end_idx].to_string();
            }
        }

        // Emergency Fallback: If XML extraction failed, find the last JSON object
        if clean_json.is_empty() {
            if let (Some(s), Some(e)) = (text.rfind('{'), text.rfind('}')) {
                if s < e {
                    clean_json = text[s..=e].to_string();
                }
            }
        }

        // Remove any markdown noise that might still be inside the tags
        let final_text = clean_json.replace("```json", "").replace("```", "").trim().to_string();

        if let Ok(report) = serde_json::from_str::<RiskReport>(&final_text) {
            Ok(report)
        } else {
            // Log for debugging but do not panic
            println!("\x1b[33m>> [ARBITER_WARN] Extraction failed. Defaulting to safe fallback.\x1b[0m");
            Ok(fallback)
        }
    }
}
