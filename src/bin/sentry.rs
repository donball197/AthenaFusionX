use std::env;
use std::fs;
use reqwest::Client;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // FIX: Use dotenvy consistently with all other binaries.
    dotenvy::dotenv().ok();

    // FIX: Use splitn(2, '=') so base64 keys containing '=' are not truncated.
    let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| {
        let env_file = fs::read_to_string(".env").unwrap_or_default();
        env_file
            .lines()
            .find(|l| l.starts_with("GEMINI_API_KEY="))
            .and_then(|l| l.splitn(2, '=').nth(1))
            .map(|v| v.trim().to_string())
            .expect("CRITICAL: GEMINI_API_KEY missing from environment and .env")
    });

    // Gather the trauma data
    let code = fs::read_to_string("src/main.rs").unwrap_or_default();
    let crash = fs::read_to_string("crash_report.txt").unwrap_or_default();

    let prompt = format!(
        "You are NEXUS, a sovereign AI engineer.\nPROTOCOL:\n1. Output FULL files inside ```rust ... ``` blocks.\n2. Fix the fatal compiler error.\n\n[CRASH LOG]\n{}\n\n[src/main.rs]\n{}",
        crash, code
    );

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}",
        api_key
    );
    let payload = json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.1, "maxOutputTokens": 4096}
    });

    println!(">> [SENTRY-BRAIN] Uplink established. Analyzing crash data...");

    let client = Client::new();
    let res: Value = client.post(&url).json(&payload).send().await?.json().await?;

    if let Some(text) = res["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        let patch = if text.contains("```rust") {
            text.split("```rust").nth(1).unwrap_or("").split("```").next().unwrap_or("").trim()
        } else if text.contains("```") {
            text.split("```").nth(1).unwrap_or("").split("```").next().unwrap_or("").trim()
        } else {
            text
        };

        if !patch.is_empty() {
            fs::write("src/main.rs", patch)?;
            println!(">> [SENTRY-BRAIN] Patch synthesized and applied to src/main.rs.");
        } else {
            println!(">> [SENTRY-BRAIN] Failed to extract patch signature.");
        }
    }

    Ok(())
}
