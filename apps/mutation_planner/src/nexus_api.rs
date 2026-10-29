use reqwest::Client;
use serde_json::json;
use std::env;
use std::error::Error;

pub async fn dispatch_to_planner(system_prompt: String) -> Result<String, Box<dyn Error>> {
    let planner_model = env::var("NEXUS_PLANNER_MODEL")
        .map_err(|_| ">> [FATAL] NEXUS_PLANNER_MODEL not set in .env -- no hardcoded default")?;

    println!(">> [NETWORK] Connecting to Nexus Mesh via API...");
    println!(">> [NETWORK] Dispatching payload to Planner: {}", planner_model);

    let client = Client::new();

    // Route to Gemini Endpoint
    if planner_model.contains("gemini") {
        let api_key =
            env::var("GEMINI_API_KEY").expect(">> [FATAL] GEMINI_API_KEY missing from .env");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            planner_model, api_key
        );

        let payload = json!({
            "contents": [{"parts": [{"text": system_prompt}]}]
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let res_json: serde_json::Value = response.json().await?;
            if let Some(text) = res_json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                println!("\n================ [ EVOLUTION REPORT ] ================\n");
                println!("{}", text);
                println!("\n======================================================\n");
                return Ok(text.to_string());
            }
        } else {
            eprintln!(">> [NETWORK ERROR] Gemini API: {}", response.text().await?);
        }
    }
    // Route to NVIDIA API Endpoint
    else {
        let api_key =
            env::var("NVIDIA_API_KEY").expect(">> [FATAL] NVIDIA_API_KEY missing from .env");
        let url = "https://integrate.api.nvidia.com/v1/chat/completions";

        let payload = json!({
            "model": planner_model,
            "messages": [
                {"role": "user", "content": system_prompt}
            ],
            "max_tokens": 2048
        });

        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let res_json: serde_json::Value = response.json().await?;
            if let Some(text) = res_json["choices"][0]["message"]["content"].as_str() {
                println!("\n================ [ EVOLUTION REPORT ] ================\n");
                println!("{}", text);
                println!("\n======================================================\n");
                return Ok(text.to_string());
            }
        } else {
            eprintln!(">> [NETWORK ERROR] NVIDIA API: {}", response.text().await?);
        }
    }

    Ok("API Call Completed but failed to parse standard response.".to_string())
}
