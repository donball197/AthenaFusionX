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
            // ⚡ FIX 2: Correctly formatting the generateContent URL
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model_endpoint.replace("models/", ""), api_key);
            let mut retries = 0;
            loop {
                let res = client.post(&url).json(payload).send().await?;
                let body: Value = res.json().await.unwrap_or_default();
                
                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    last_err = anyhow!("API Error: {}", err);
                    
                    // ⚡ THE MISSING SUTURE: Trap transient errors and RETRY with a backoff!
                    if err.contains("Internal error") || err.contains("high demand") || err.contains("quota") || err.contains("rate limit") {
                        retries += 1;
                        if retries > 3 {
                            println!("\x1b[31m>> [API FATAL] {} exhausted retries. Falling back...\x1b[0m", short_name);
                            break; // Exhausted retries, move to next model in the cascade
                        }
                        println!("\x1b[33m>> [API RETRY] {} hit transient fault. Backing off 4s (Attempt {}/3)...\x1b[0m", short_name, retries);
                        tokio::time::sleep(Duration::from_secs(4)).await;
                        continue;
                    } else {
                        println!("\x1b[31m>> [API ERROR] {} returned fatal error: {}\x1b[0m", short_name, err);
                        break; // Fatal error (e.g. bad API key), move to next model immediately
                    }
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

    // ⚡ FIX 3: Removed responseMimeType from ALL Gemma API calls so it doesn't 500 error out.

    pub async fn generate_hypotheses(client: &Client, api_key: &str, problem: &str, world_context: &str) -> Result<Vec<String>> {
        let prompt = format!("You are AthenaFusionX. Analyze this directive.\nDIRECTIVE: {}\nWORLD CONTEXT: {}\nGenerate 3 distinct, actionable strategies. OUTPUT ONLY A JSON ARRAY OF STRINGS. No markdown.", problem, world_context);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 2048 }});
        let text = Self::safe_api_call(client, api_key, &get_heavy_models(), &payload).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn propose_action(client: &Client, api_key: &str, mission: &str, strategy: &str, evidence_so_far: &str) -> Result<String> {
        let prompt = format!("You are the Action Strategist.\nORIGINAL DIRECTIVE (stay strictly within its scope -- do not investigate other files unless the directive or evidence explicitly points there): {}\nSTRATEGY: {}\nEVIDENCE GATHERED SO FAR:\n{}\nPropose the exact next step to fulfill the strategy. CRITICAL RULE: NEVER run `find /` or search the root directory! Only search the current working directory (`.`). OUTPUT ONLY YOUR PROPOSED NEXT STEP IN PLAIN TEXT.", mission, strategy, evidence_so_far);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.2, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &get_light_models(), &payload).await
    }

    pub async fn criticize_proposal(client: &Client, api_key: &str, strategy: &str, proposed_action: &str, world_context: &str) -> Result<CriticResponse> {
        let prompt = format!("You are the Adversarial Critic.\nSTRATEGY: {}\nPROPOSED ACTION: {}\nWORLD CONTEXT: {}\nEvaluate if this action is safe and logically sound. \nCRITICAL RULE: Always allow 'read_file' and 'find' operations even if the path is not visible in the WORLD CONTEXT. A failed read is safe exploration. Do not veto read operations for missing paths.\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", strategy, proposed_action, world_context);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024 }});
        let text = Self::safe_api_call(client, api_key, &get_heavy_models(), &payload).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn translate_to_action(client: &Client, api_key: &str, strategy: &str, proposed_action: &str) -> Result<String> {
        let prompt = format!(
            "You are the Action Translator.\nSTRATEGY: {}\nPROPOSED ACTION: {}\nCRITICAL RULE: Translate the PROPOSED ACTION into exactly ONE JSON Array using the explicit JSON object schema below:\n\
             VALID JSON ACTIONS (MUST MATCH RUST ENUM EXACTLY):\n\
             1. [{{ \"type\": \"execute_command\", \"command\": \"<bash_command>\", \"reasoning\": \"...\" }}]\n\
             2. [{{ \"type\": \"read_file\", \"path\": \"<filepath>\", \"reasoning\": \"...\" }}]\n\
             3. [{{ \"type\": \"write_file\", \"path\": \"<filepath>\", \"content\": \"<data>\", \"reasoning\": \"...\" }}]\n\
             4. [{{ \"type\": \"diff_patch\", \"path\": \"<filepath>\", \"search\": \"<search>\", \"replace\": \"<replace>\", \"reasoning\": \"...\" }}]\n\
             5. [{{ \"type\": \"patch_function\", \"path\": \"<filepath>\", \"function_name\": \"<name>\", \"new_body\": \"<full source>\", \"reasoning\": \"...\" }}]\n\
             6. [{{ \"type\": \"halt\", \"reasoning\": \"...\" }}]\n\
             NOTE: Do NOT use types like 'find_file', 'ls_tree', or 'grep_code'. If you need to search or list, use 'execute_command' (e.g. command: 'ls -F' or command: 'find .').\n\
             Select ONLY the relevant action object and enclose it in a JSON array. Do not output anything else.", 
            strategy, proposed_action
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 2048 }});
        Self::safe_api_call(client, api_key, &get_light_models(), &payload).await
    }

    pub async fn evaluate_evidence(client: &Client, api_key: &str, strategy: &str, evidence: &str) -> Result<CriticResponse> {
        let truncated_evidence: String = evidence.chars().take(8000).collect();
        let prompt = format!("You are the Scientific Evaluator.\nSTRATEGY: {}\nEVIDENCE GATHERED:\n{}\nDoes this evidence progress the strategy? If the evidence shows a successful compilation or successful file creation, give a positive modifier. If it is neutral observation, return 0.0.\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", strategy, truncated_evidence);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024 }});
        let text = Self::safe_api_call(client, api_key, &get_heavy_models(), &payload).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))
    }
}
