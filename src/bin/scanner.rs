use reqwest::Client;
use std::process::Command;
use serde_json::json;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the .env file to grab the GEMMA_API_KEY
    dotenv().ok();
    
    println!(">> [SCANNER] Mapping Infrastructure Metadata...");

    // 1. SURGICAL METADATA COLLECTION:
    // We target the root and vault, specifically ignoring the /target bloat.
    let output = Command::new("sh")
        .arg("-c")
        .arg("du -sh ./* && find . -maxdepth 2 -not -path '*/.*' -not -path './target/*' | xargs file")
        .output()?;

    let metadata = String::from_utf8_lossy(&output.stdout);

    // 2. CONDUCTOR HANDSHAKE:
    let api_key = std::env::var("GEMMA_API_KEY")
        .expect("ERROR: GEMMA_API_KEY not found in .env");
    
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
    
    let client = Client::new();
    let prompt = format!(
        "As the Nexus Infrastructure Auditor, analyze this build path metadata:\n{}\n\
        Verify that the .litertlm models in the /vault are correctly positioned for ARM64 inference. \
        Check for any logical fragmentation or redundant dependencies.",
        metadata
    );

    let payload = json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": { "temperature": 0.1 }
    });

    println!(">> [SCANNER] Sending Metadata to 27B Conductor for Analysis...");
    
    let resp = client.post(url).json(&payload).send().await?;
    let body: serde_json::Value = resp.json().await?;
    
    let analysis = body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("AUDIT_STALL: The Conductor did not return a report.");

    println!("\n>> [AUDIT REPORT - 15th LAYER]:\n{}", analysis);

    Ok(())
}
