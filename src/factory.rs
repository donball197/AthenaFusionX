/**
 * Project Nexus Singularity - Agent Factory
 * Dynamically scout and categorize available neural cores.
 */
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerativeModel {
    pub name: String,
    pub input_token_limit: u32,
    pub output_token_limit: u32,
    pub description: String,
    pub tier: String,
}

pub struct AgentFactory;

impl AgentFactory {
    pub async fn fetch_models(api_key: &str) -> Result<Vec<GenerativeModel>> {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", api_key);
        let client = Client::new();
        let resp = client.get(url).send().await?.json::<serde_json::Value>().await?;
        
        let mut models = Vec::new();
        if let Some(models_array) = resp["models"].as_array() {
            for m in models_array {
                let name = m["name"].as_str().unwrap_or("").replace("models/", "");
                
                // Categorize for the UI
                let tier = if name.contains("pro") { "ORCHESTRATOR" }
                          else if name.contains("flash") { "COURIER" }
                          else if name.contains("gemma") { "RESEARCHER" }
                          else if name.contains("veo") || name.contains("imagen") { "VISUALIZER" }
                          else { "UTILITY" };

                models.push(GenerativeModel {
                    name,
                    input_token_limit: m["inputTokenLimit"].as_u64().unwrap_or(0) as u32,
                    output_token_limit: m["outputTokenLimit"].as_u64().unwrap_or(0) as u32,
                    description: m["description"].as_str().unwrap_or("No documentation available.").to_string(),
                    tier: tier.to_string(),
                });
            }
        }
        
        // Sort by Input Limit (High to Low)
        models.sort_by(|a, b| b.input_token_limit.cmp(&a.input_token_limit));
        Ok(models)
    }
}
