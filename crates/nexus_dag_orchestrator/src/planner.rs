use serde::Deserialize;
use serde_json::json;
use std::env;
use std::error::Error;

#[derive(Debug, Deserialize)]
pub struct PlannedTask {
    pub id: String,
    pub description: String,
    pub required_tier: String,
    pub dependencies: Vec<String>,
}

pub struct DagPlanner;

impl DagPlanner {
    fn resolve_provider() -> (String, String, String) {
        let model = env::var("NEXUS_PLANNER_MODEL").unwrap_or_else(|_| "nexus".to_string());
        if let Ok(gateway) = env::var("NEXUS_API_GATEWAY") {
            let api_key = env::var("NEXUS_API_KEY").unwrap_or_else(|_| "local-node".to_string());
            let endpoint = format!("{}/v1/chat/completions", gateway.trim_end_matches("/"));
            (endpoint, api_key, model)
        } else if model.contains("gemini") {
            (
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                    .to_string(),
                env::var("GEMINI_API_KEY").unwrap_or_default(),
                model,
            )
        } else {
            (
                "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
                env::var("NVIDIA_API_KEY").unwrap_or_default(),
                model,
            )
        }
    }

    pub fn generate_dag(objective: &str) -> Result<Vec<PlannedTask>, Box<dyn Error>> {
        let (endpoint, api_key, model) = Self::resolve_provider();

        if api_key.is_empty() {
            return Err("Planner API key is missing from .env".into());
        }

        let mut system_prompt = String::from("You are the Nexus Executive Cortex. Break the user's objective into a Directed Acyclic Graph (DAG) of discrete micro-agent tasks. Output ONLY a raw JSON array of objects. Do not include markdown formatting, backticks, or conversational text. Schema: [{\"id\": \"task_1\", \"description\": \"Action required...\", \"required_tier\": \"light\" or \"heavy\", \"dependencies\": [\"array_of_task_ids_this_waits_on\"]}]");

        let mentions_file_output = objective.contains(".md")
            || objective.contains(".json")
            || objective.contains(".txt")
            || objective.contains(".csv")
            || objective.to_lowercase().contains("write to")
            || objective.to_lowercase().contains("save to")
            || objective.to_lowercase().contains("output to");

        if mentions_file_output {
            system_prompt.push_str("\n\nCRITICAL RULE 1: You MUST preserve ALL specific file paths, output formats, and tool names requested by the user in your task descriptions! If the user asks to write to a specific .md or .json file, the final task description MUST explicitly state to write that exact file.");
        }

        let payload = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": format!("Objective: {}", objective) }
            ],
            "temperature": 0.1,
            "max_tokens": 2048
        });

        let response = ureq::post(&endpoint)
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(payload)?;

        let response_json: serde_json::Value = response.into_json()?;
        let mut text = match response_json["choices"][0]["message"]["content"].as_str() {
            Some(t) if !t.trim().is_empty() => t.to_string(),
            _ => {
                eprintln!("[planner] empty/missing content — treating as failure");
                return Err("planner: missing choices[0].message.content".into());
            }
        };

        // Sanitize LLM formatting drift
        text = text.replace("```json", "").replace("```", "").trim().to_string();

        let tasks: Vec<PlannedTask> = serde_json::from_str(&text)?;
        Ok(tasks)
    }
}
