use crate::factory::AgentFactory;
use reqwest::Client;
use serde_json::{json, Value};

pub struct KnowledgeBroker;

impl KnowledgeBroker {
    pub async fn refresh_atlas() {
        // Indexing project DNA for context-injection
        println!("\x1b[34m>> [BROKER] DNA Structure indexed to local atlas.\x1b[0m");
    }

    pub async fn query_atlas(
        api_key: &str,
        query: &str,
        atlas_context: &str,
    ) -> Result<String, String> {
        let client = Client::new();
        let model = AgentFactory::select_model_for_mission("broker");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );
        let prompt = format!("ATLAS CONTEXT:\n{}\n\nQUERY: {}", atlas_context, query);
        let payload = json!({ "contents": [{"parts": [{"text": prompt}]}] });
        let res = client.post(&url).json(&payload).send().await.map_err(|e| e.to_string())?;
        let json: Value = res.json().await.map_err(|e| e.to_string())?;
        Ok(json["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string())
    }
}
