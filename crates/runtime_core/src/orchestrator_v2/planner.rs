use crate::orchestrator_v2::task_graph::{TaskGraph, TaskNode, TaskStatus};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

pub struct PlannerAgent;

impl PlannerAgent {
    pub async fn generate_plan(client: &Client, api_key: &str, directive: &str) -> Result<TaskGraph> {
        let system_instruction = 
            "You are a headless JSON API. You MUST output ONLY a valid, raw JSON array. \
            Do NOT use markdown formatting like ```json. Do NOT write any conversational text. \
            Do NOT explain your steps. Your output must start exactly with `[` and end with `]`. CRITICAL RULE: If the directive involves modifying Rust code, your DAG MUST include a final task to verify integrity by running `cargo clippy --workspace --all-targets -j 1 -- -D warnings`. \
            CRITICAL: The ONLY valid fields for each object are 'id', 'objective', 'deps', 'required_agent', and 'success_criteria'. \
            DO NOT output any other fields like 'params', 'required_files', or 'Architectural_Strategies'. Embed all details into the 'objective' string.";

        // ⚡ FIX 1: Removed the dangling "role": "model" pre-fill that crashes the Gemini API.
        // Instead, we force the bracket requirement in the final user prompt.
        let payload = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": system_instruction }]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "DIRECTIVE: Audit the codebase for secrets.\nUse world model knowledge. Prefer targeted reads over broad grep." }]
                },
                {
                    "role": "model",
                    "parts": [{ "text": "[\n  {\n    \"id\": \"task_1\",\n    \"objective\": \"Scan src/ for API keys\",\n    \"deps\": [],\n    \"required_agent\": \"AuditorAgent\",\n    \"success_criteria\": [\"Scan completes\"]\n  }\n]" }]
                },
                {
                    "role": "user",
                    "parts": [{ "text": format!("DIRECTIVE: {}\n\nSTART YOUR RESPONSE WITH `[`", directive) }]
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
        
        let url = format!("{}/v1beta/models/gemini-3.1-flash-lite:generateContent?key={}", base_url, api_key);

        let mut attempts = 0;
        let mut last_error = String::new();

        while attempts < 3 {
            attempts += 1;
            match client.post(&url).json(&payload).send().await {
                Ok(res) => {
                    let status = res.status();
                    let body: Value = res.json().await?;
                    
                    if status == 429 || body.pointer("/error/status").and_then(|s| s.as_str()) == Some("RESOURCE_EXHAUSTED") {
                        tracing::warn!("[PLANNER] Quota Exhausted! Sleeping for 15 seconds...");
                        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                        continue;
                    }

                    if let Some(err) = body.pointer("/error/message") {
                        return Err(anyhow!("API Error: {}", err));
                    }

                    let mut text = body.pointer("/candidates/0/content/parts/0/text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !text.starts_with('[') {
                        text = format!("[{}", text);
                    }

                    let array_str = Self::extract_json_array(&text)
                        .ok_or_else(|| anyhow!("No valid JSON array found in response:\n{}", text))?;
                    
                    let nodes: Vec<TaskNode> = serde_json::from_str(&array_str)
                        .map_err(|e| anyhow!("DAG Parse Fracture: {}\nRaw Extracted: {}", e, array_str))?;

                    if nodes.is_empty() {
                        return Err(anyhow!("Model returned an empty task array. Raw Output: {}", text));
                    }

                    let mut graph = TaskGraph::new();
                    for mut node in nodes {
                        node.status = TaskStatus::Pending; 
                        graph.nodes.insert(node.id.clone(), node);
                    }

                    return Ok(graph);
                }
                Err(e) => {
                    last_error = e.to_string();
                    tracing::warn!("[PLANNER] Transport drop on attempt {}/3. Retrying in 2s...", attempts);
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
        Err(anyhow!("FATAL TRANSPORT FRACTURE after 3 attempts: {}", last_error))
    }

    fn extract_json_array(text: &str) -> Option<String> {
        let start = text.find('[')?;
        let mut depth = 0;
        for (i, c) in text[start..].char_indices() {
            match c {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(text[start..=start + i].to_string());
                    }
                }
                _ => {}
            }
        }

        let partial = &text[start..];
        let mut end_idx = partial.len();
        
        while let Some(last_close) = partial[..end_idx].rfind('}') {
            let candidate = format!("{}]", &partial[..=last_close]);
            let clean_candidate: String = candidate.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect();

            if serde_json::from_str::<serde_json::Value>(&clean_candidate).is_ok() {
                return Some(clean_candidate);
            }
            end_idx = last_close;
        }
        
        None
    }
}
