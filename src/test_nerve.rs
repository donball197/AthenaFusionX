use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY not set");
    let client = Client::new();

    println!(">> [NEXUS] PULSING GEMMA 3 27B-IT VIA RUST...");

    let response = client
        .post(format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key))
        .json(&json!({
            "contents": [{
                "parts": [{
                    "text": "System: Nexus Sovereign Kernel. Task: Spawn micro-agent logic for Hive Units 12 and 44. Output status as JSON."
                }]
            }]
        }))
        .send()
        .await?;

    let body = response.text().await?;
    println!(">> [NEXUS] BRAIN RESPONSE SEATED:\n{}", body);

    Ok(())
}
