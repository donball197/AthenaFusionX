use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{sleep, Duration};
use crate::model_config::{get_light_models, get_heavy_models};

#[derive(Debug, Serialize, Deserialize)]
pub struct CriticResponse {
    pub critique: String,
    pub confidence_modifier: f32,
}

pub struct EpistemicEngine;

impl EpistemicEngine {
    async fn safe_api_call(client: &Client, api_key: &str, models: &[String], payload: &Value) -> Result<String> {
        let mut last_err = anyhow!("No models available in cascade.");
        for model_endpoint in models {
            let short_name = model_endpoint.split(':').next().unwrap_or("Model");
            // ⚡ FIX: Added :generateContent to URL builder to prevent 404s on fallback models
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model_endpoint, api_key);
            let mut retries = 0;
            loop {
                let res = client.post(&url).json(payload).send().await?;
                let body: Value = res.json().await.unwrap_or_default();
                
                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    println!("\x1b[31m>> [API ERROR] {} returned: {}\x1b[0m", short_name, err);
                    last_err = anyhow!("API Error: {}", err);
                    continue;
                }

                let mut final_text = String::new();
                if let Some(parts) = body.pointer("/candidates/0/content/parts").and_then(|v| v.as_array()) {
                    for part in parts {
                        if part.get("thought").and_then(|t| t.as_bool()).unwrap_or(false) { continue; }
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) { final_text.push_str(text); }
                    }
                }
                
                if final_text.trim().is_empty() {
                    retries += 1;
                    if retries > 2 { break; }
                    println!("\x1b[33m>> [API THROTTLE] {} hit rate limit. Backing off 3s...\x1b[0m", short_name);
                    sleep(Duration::from_secs(3)).await;
                    continue;
                }
                return Ok(final_text.replace("```json", "").replace("```", "").trim().to_string());
            }
        }
        Err(anyhow!("Model Cascade Exhausted. Final error: {}", last_err))
    }

    pub async fn generate_hypotheses(client: &Client, api_key: &str, problem: &str, world_context: &str) -> Result<Vec<String>> {
        let prompt = format!("You are AthenaFusionX. Analyze this directive.\nDIRECTIVE: {}\nWORLD CONTEXT: {}\nGenerate 3 distinct, actionable strategies. OUTPUT ONLY A JSON ARRAY OF STRINGS. No markdown.", problem, world_context);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.4, "maxOutputTokens": 2048, "responseMimeType": "application/json" }});
        let text = Self::safe_api_call(client, api_key, &get_heavy_models(), &payload).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn propose_action(client: &Client, api_key: &str, strategy: &str, evidence_so_far: &str, world_context: &str) -> Result<String> {
        let prompt = format!(
            "You are the Action Strategist.\n\
             STRATEGY: {}\n\
             WORLD CONTEXT (Available Files):\n{}\n\
             EVIDENCE GATHERED SO FAR:\n{}\n\
             Propose the exact next physical step to fulfill the strategy.\n\
             CRITICAL RULES:\n\
             1. DO NOT guess or hallucinate file paths. ONLY target files listed in the WORLD CONTEXT or EVIDENCE.\n\
             2. Propose a concrete, executable action (e.g., 'Read the file src/main.rs', or 'Execute `ls -la third_party/`').\n\
             3. If you need to see inside a specific directory, propose executing an `ls` command on it.\n\
             4. Do not propose abstract concepts like `[T; 8]`. Output the literal action intent.\n\
             OUTPUT ONLY YOUR PROPOSED NEXT STEP IN PLAIN TEXT.",
            strategy, world_context, evidence_so_far
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &get_light_models(), &payload).await
    }

    pub async fn criticize_proposal(client: &Client, api_key: &str, strategy: &str, proposed_action: &str, world_context: &str) -> Result<CriticResponse> {
        let prompt = format!("You are the Adversarial Critic.\nSTRATEGY: {}\nPROPOSED ACTION: {}\nWORLD CONTEXT: {}\nEvaluate if this action is safe and logically sound. OUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", strategy, proposed_action, world_context);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024, "responseMimeType": "application/json" }});
        let text = Self::safe_api_call(client, api_key, &get_heavy_models(), &payload).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn translate_to_action(client: &Client, api_key: &str, strategy: &str, proposed_action: &str) -> Result<String> {
        // --- ⚡ LOCAL HEURISTIC FAST-PATHS (0 RAM, 0 API CALLS) ---
        let action_lower = proposed_action.to_lowercase();
        
        // 1. Command Execution Fast-Path
        if action_lower.contains("execute") {
            if let Some(start) = proposed_action.find('`') {
                let rest = &proposed_action[start + 1..];
                if let Some(end) = rest.find('`') {
                    let cmd = &rest[..end];
                    let json_out = serde_json::json!([{
                        "type": "execute_command",
                        "command": cmd,
                        "reasoning": "Local fast-path translation."
                    }]);
                    println!("\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\x1b[0m");
                    return Ok(json_out.to_string());
                }
            }
        }

        // 2. File Read Fast-Path
        if action_lower.starts_with("read the file") {
            let parts: Vec<&str> = proposed_action.split_whitespace().collect();
            if parts.len() >= 4 {
                let path = parts[3]; // Typically: "Read" "the" "file" "path/to/file"
                let json_out = serde_json::json!([{
                    "type": "read_file",
                    "path": path,
                    "reasoning": "Local fast-path translation."
                }]);
                println!("\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\x1b[0m");
                return Ok(json_out.to_string());
            }
        }
        // --- END FAST-PATHS ---

        let prompt = format!(
            "You are the Action Translator.\nSTRATEGY: {}\nPROPOSED ACTION: {}\nCRITICAL RULE: Translate the PROPOSED ACTION into exactly ONE JSON Array using the schema below.\n\
             VALID JSON ACTIONS (MUST MATCH RUST ENUM EXACTLY):\n\
             [{{\"type\": \"execute_command\", \"command\": \"...\", \"reasoning\": \"...\"}}]\n\
             [{{\"type\": \"read_file\", \"path\": \"...\", \"reasoning\": \"...\"}}]\n\
             [{{\"type\": \"write_file\", \"path\": \"...\", \"content\": \"...\", \"reasoning\": \"...\"}}]\n\
             [{{\"type\": \"diff_patch\", \"path\": \"...\", \"search\": \"...\", \"replace\": \"...\", \"reasoning\": \"...\"}}]\n\
             [{{\"type\": \"patch_function\", \"path\": \"...\", \"function_name\": \"...\", \"new_body\": \"...\", \"reasoning\": \"...\"}}]\n\
             Select ONLY the relevant action object and enclose it in a JSON array. DO NOT output markdown. NO conversational text.", 
            strategy, proposed_action
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.0, "maxOutputTokens": 2048, "responseMimeType": "application/json" }});
        Self::safe_api_call(client, api_key, &get_light_models(), &payload).await
    }

    pub async fn evaluate_evidence(client: &Client, api_key: &str, strategy: &str, evidence: &str) -> Result<CriticResponse> {
        let truncated_evidence: String = evidence.chars().take(4000).collect();
        let prompt = format!("You are the Scientific Evaluator.\nSTRATEGY: {}\nEVIDENCE GATHERED:\n{}\nDoes this evidence progress the strategy? If the evidence shows a successful compilation or successful file creation, give a positive modifier. If the evidence provides valuable discovery, context, or code required to complete the strategy, give a positive modifier (e.g., 0.1 to 0.2).\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", strategy, truncated_evidence);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024, "responseMimeType": "application/json" }});
        let text = Self::safe_api_call(client, api_key, &get_heavy_models(), &payload).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))
    }
}
