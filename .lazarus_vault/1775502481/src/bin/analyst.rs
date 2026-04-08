use reqwest::Client;
use serde_json::json;
use std::env;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Manually load .env file into the process environment
    dotenvy::dotenv().ok();
    
    let api_key = env::var("GEMINI_API_KEY").expect("CRITICAL: GEMINI_API_KEY not found in .env");
    let client = Client::new();

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: analyst <file_path> <error_msg>");
        return Ok(());
    }

    let file_path = &args[1];
    let error_msg = &args[2];
    let code = fs::read_to_string(file_path)?;

    let prompt = format!(
        "FIX THIS RUST CODE. Error: {}\n\nCODE:\n{}\n\nOutput ONLY the corrected code. No explanations. No markdown blocks.",
        error_msg, code
    );

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
    
    let payload = json!({
        "contents": [{ "parts": [{ "text": prompt }] }]
    });

    println!(">> Sending to Google for healing...");
    let res = client.post(&url).json(&payload).send().await?;
    let body: serde_json::Value = res.json().await?;

    if let Some(fixed_code) = body["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        // Strip any accidental markdown formatting the AI might add
        let clean_code = fixed_code.replace("```rust", "").replace("```", "").trim().to_string();
        fs::write(file_path, clean_code)?;
        println!(">> [SUCCESS] {} has been healed.", file_path);
    } else {
        println!(">> [FAILURE] API response was empty or malformed.");
        println!("Full Response: {:?}", body);
    }

    Ok(())
}
