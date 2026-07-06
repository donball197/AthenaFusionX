use crate::orchestrator_v2::task_graph::{TaskGraph, TaskNode, TaskStatus};
use crate::model_config::get_light_models;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

pub struct PlannerAgent;

impl PlannerAgent {
    // ⚡ V4 ISOLATOR: Bulletproof extraction of JSON Arrays from hallucinated markdown 
    fn isolate_json_array(raw: &str) -> Option<String> {
        let s = raw.replace("```json", "").replace("```", "").trim().to_string();
        if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
            if end > start {
                return Some(s[start..=end].to_string());
            }
        }
        None
    }

    pub async fn generate_plan(client: &Client, api_key: &str, directive: &str) -> Result<TaskGraph> {
        let system_instruction = 
            "You are a headless JSON API. You MUST output ONLY a valid, raw JSON array. \
            Do NOT use markdown formatting like ```json. Do NOT write any conversational text. \
            Do NOT explain your steps. Your output must start exactly with `[` and end with `]`. \
            CRITICAL RULE: If the directive involves modifying Rust code, your DAG MUST include a final task to verify integrity by running `cargo clippy --workspace --all-targets -j 1 -- -D warnings`. \
            CRITICAL: The ONLY valid fields for each object are 'id', 'objective', 'deps', 'required_agent', and 'success_criteria'. \
            Embed all details into the 'objective' string.";

        // Use r#""# to prevent escape sequence destruction
        let payload = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": system_instruction }]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "DIRECTIVE: Audit the codebase for secrets. Use world model knowledge." }]
                },
                {
                    "role": "model",
                    "parts": [{ "text": r#"
[
  {
    "id": "task_1",
    "objective": "Scan src/ for API keys",
    "deps": [],
    "required_agent": "AuditorAgent",
    "success_criteria": ["Scan completes"]
  }
]"# 
                    }]
                },
                {
                    "role": "user",
                    "parts": [{ "text": format!("DIRECTIVE: {}", directive) }]
                }
            ],
            "generationConfig": { 
                "temperature": 0.1, 
                "topK": 40,
                "topP": 0.95,
                "maxOutputTokens": 8192
            }
        });

        let base_url = std::env::var("NEXUS_API_GATEWAY")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string());
        
        let mut last_error = String::new();
        
        // ⚡ CASCADE LOOP: Failover seamlessly through light models if one times out
        for model_name in get_light_models() {
            let url = format!("{}/v1beta/models/{}:generateContent?key={}", base_url.trim(), model_name, api_key.trim());
            
            let mut attempts = 0;
            while attempts < 2 {
                attempts += 1;
                match client.post(&url).json(&payload).send().await {
                    Ok(res) => {
                        let status = res.status();
                        let body: Value = res.json().await.unwrap_or_default();
                        
                        if status == 429 || body.pointer("/error/status").and_then(|s| s.as_str()) == Some("RESOURCE_EXHAUSTED") {
                            tracing::warn!("[PLANNER] {} Quota Exhausted! Cascading...", model_name);
                            last_error = format!("{} Rate Limited", model_name);
                            break; 
                        }

                        if let Some(err) = body.pointer("/error/message") {
                            last_error = err.to_string();
                            break; 
                        }

                        let text = body.pointer("/candidates/0/content/parts/0/text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        // ⚡ SCRUB AND ISOLATE
                        let array_str = match Self::isolate_json_array(&text) {
                            Some(s) => s,
                            None => {
                                last_error = "No valid JSON array found in response.".to_string();
                                continue; 
                            }
                        };
                        
                        let nodes: Vec<TaskNode> = match serde_json::from_str(&array_str) {
                            Ok(n) => n,
                            Err(e) => {
                                last_error = format!("DAG Parse Fracture: {}\nRaw Extracted: {}", e, array_str);
                                continue; 
                            }
                        };

                        if nodes.is_empty() {
                            last_error = "Model returned an empty task array.".to_string();
                            continue;
                        }

                        let mut graph = TaskGraph::new();
                        for mut node in nodes {
                            node.status = TaskStatus::Pending; 
                            graph.nodes.insert(node.id.clone(), node);
                        }

                        return Ok(graph);
                    }
                    Err(e) => {
                        last_error = format!("Network failure: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
        Err(anyhow!("FATAL CASCADE EXHAUSTION: All models failed. Last error: {}", last_error))
    }
}
