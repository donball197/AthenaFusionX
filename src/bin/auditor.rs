use reqwest::Client;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY missing.");
    let client = Client::new();
    
    // ASSIGNED MODEL: Gemma-3-27b-it for high-reasoning audit
    let model_id = "gemma-3-27b-it"; 
    
    let args: Vec<String> = env::args().collect();
    let query = args.get(1).map(|s| s.as_str()).unwrap_or("Perform system audit");

    println!(">> [AUDITOR AGENT] Brain: {} | Task: {}", model_id, query);

    let system_prompt = format!("You are the Auditor Agent. Analyze this specific request with extreme attention to detail: {}", query);
    
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model_id, api_key);
    let payload = json!({ "contents": [{ "parts": [{ "text": system_prompt }] }] });

    let res = client.post(&url).json(&payload).send().await?.json::<serde_json::Value>().await?;
    if let Some(text) = res["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        println!("\nAUDIT REPORT:\n{}", text);
    }
    Ok(())
}
