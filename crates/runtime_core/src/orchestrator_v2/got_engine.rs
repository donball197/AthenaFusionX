use reqwest::Client;
use serde_json::{json, Value};

pub struct GraphOfThoughts;

impl GraphOfThoughts {
    pub async fn explore_strategies(client: &Client, api_key: &str, directive: &str) -> String {
        let prompt = format!(
            "You are the GraphOfThoughts Engine. Explore 3 distinct strategies for the following directive.\n\
             Evaluate the pros and cons of each, then select the optimal path and execute it conceptually.\n\
             DIRECTIVE: {}\n\n\
             CRITICAL: After detailing Phase 3, you MUST output a JSON array of AgentAction commands to physically execute your selected strategy.\n\
             MANDATORY SCHEMA:\n\
             [\n\
               {{ \"type\": \"execute_command\", \"command\": \"...\", \"reasoning\": \"...\" }},\n\
               {{ \"type\": \"read_file\", \"path\": \"...\", \"reasoning\": \"...\" }},\n\
               {{ \"type\": \"write_file\", \"path\": \"...\", \"content\": \"...\", \"reasoning\": \"...\" }},\n\
               {{ \"type\": \"diff_patch\", \"path\": \"...\", \"search\": \"...\", \"replace\": \"...\", \"reasoning\": \"...\" }},\n\
               {{ \"type\": \"patch_function\", \"path\": \"...\", \"function_name\": \"...\", \"new_body\": \"...\", \"reasoning\": \"...\" }},\n\
               {{ \"type\": \"halt\", \"reasoning\": \"...\" }}\n\
             ]\n\
             Output the JSON array at the very end of your response.", directive
        );

        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024 }
        });

        // ⚡ MODEL CASCADE: Try Heavy Reasoning first, fallback to Fast/Stable
        let models = ["gemma-4-31b-it", "gemini-3.1-flash-lite"];

        for model in models {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model, api_key
            );

            for _attempt in 1..=2 {
                match client.post(&url).json(&payload).send().await {
                    Ok(res) => {
                        if let Ok(body) = res.json::<Value>().await {
                            if let Some(err) =
                                body.pointer("/error/message").and_then(|v| v.as_str())
                            {
                                println!(
                                    "\x1b[33m>> [GOT ENGINE] {} fault: {}. Falling back...\x1b[0m",
                                    model, err
                                );
                                break;
                            }

                            let mut final_text = String::new();
                            if let Some(parts) = body
                                .pointer("/candidates/0/content/parts")
                                .and_then(|v| v.as_array())
                            {
                                for part in parts {
                                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                        if part
                                            .get("thought")
                                            .and_then(|t| t.as_bool())
                                            .unwrap_or(false)
                                        {
                                            final_text.push_str(&format!("\x1b[90m>>> [INTERNAL REASONING]\n{}\n<<<\x1b[0m\n", text.trim()));
                                        } else {
                                            final_text.push_str(text);
                                            final_text.push('\n');
                                        }
                                    }
                                }
                            }
                            if !final_text.is_empty() {
                                return final_text;
                            }
                        }
                    }
                    Err(e) => {
                        println!(
                            "\x1b[31m>> [GOT ENGINE] Network fault on {}: {}\x1b[0m",
                            model, e
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
        "GraphOfThoughts execution failed. All models in the cascade returned errors or timed out."
            .to_string()
    }
}
