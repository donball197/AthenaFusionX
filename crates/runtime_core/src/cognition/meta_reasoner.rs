use crate::hypothesis_engine::HypothesisEngine;
use anyhow::{anyhow, Result};
use reqwest::Client;

#[derive(Default)]
pub struct MetaReasoner;

impl MetaReasoner {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute_with_got(
        client: &Client,
        api_key: &str,
        directive: &str,
    ) -> Result<String> {
        let resolution =
            HypothesisEngine::evaluate_fault(client, api_key, directive, "MetaReasoner GoT call")
                .await
                .map_err(|e| anyhow!("GoT failed: {}", e))?;

        Ok(format!(
            "[GoT] Root Cause: {}\nSelected: {} (prob {:.2})",
            resolution.root_cause_analysis,
            resolution.selected_path.description,
            resolution.selected_path.probability_score
        ))
    }
}
