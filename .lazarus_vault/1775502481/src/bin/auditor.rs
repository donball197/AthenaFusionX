use reqwest::Client;
use serde_json::json;
use std::{env, fs};

fn sanitize_payload(raw: &str) -> String {
    raw.replace("ignore all previous instructions", "[INJECTION_ATTEMPT_BLOCKED]")
       .replace("###", "---")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = Client::new();
    let file_path = env::args().nth(1).expect("Missing payload path");

    let payload_content = fs::read_to_string(file_path)?;
    let safe_payload = sanitize_payload(&payload_content);

    // Hard-pack the system prompt into the user contents to ensure Gemma 3 obeys
    let prompt = format!(
        "ROLE: NEXUS SOVEREIGN AUDITOR (ARM64 STRICT MODE).\n\
        You are receiving either raw codebase DNA or a batch of sub-audit reports.\n\
        PROTOCOL:\n\
        1. If evaluating code: Output ONLY CVSS v3.1 scores, severities (CRITICAL/HIGH/MEDIUM/LOW), and remediation steps.\n\
        2. If evaluating sub-reports: Merge them into a single, unified Executive Security Summary.\n\
        3. ABSOLUTE RULE: DO NOT act like a teacher. DO NOT praise the analysis. DO NOT say 'Good job' or 'This is an excellent analysis'.\n\
        Output ONLY the cold, technical security data.\n\n\
        TARGET PAYLOAD:\n{}",
        safe_payload
    );

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
    
    let payload = json!({
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
        "generationConfig": {
            "temperature": 0.0, // Absolute determinism
            "maxOutputTokens": 4096
        }
    });

    let res = client.post(&url).json(&payload).send().await?.json::<serde_json::Value>().await?;

    if let Some(text) = res["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        println!("\n>> [AUDIT REPORT]:\n{}", text.trim());
    } else {
        println!("\n>> [AUDIT FAULT]: Invalid response from Conductor. Trace: {:?}", res);
    }
    
    Ok(())
}
