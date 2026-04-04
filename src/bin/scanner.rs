use reqwest::Client;
use serde_json::json;
use std::process::Command;
use dotenvy::dotenv;

/// FIX: Replaced `Command::new("sh").arg("-c").arg("du ... && find ...")` with two
/// explicit, separate Command invocations. The previous sh -c pattern is safe with
/// static strings but becomes a shell injection vector the moment any variable is
/// interpolated. Explicit args eliminate that risk entirely and are easier to audit.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    println!(">> [SCANNER] Mapping Infrastructure Metadata...");

    // 1a. Disk usage — explicit args, no shell interpolation
    let du_output = Command::new("du")
        .args(["-sh", "--exclude=target", "--exclude=.git", "--exclude=node_modules", "."])
        .output()?;
    let du_text = String::from_utf8_lossy(&du_output.stdout);

    // 1b. File type inventory — explicit args
    let find_output = Command::new("find")
        .args([
            ".",
            "-maxdepth", "2",
            "-not", "-path", "*/.*",
            "-not", "-path", "./target/*",
            "-not", "-path", "./node_modules/*",
        ])
        .output()?;
    let find_text = String::from_utf8_lossy(&find_output.stdout);

    let metadata = format!("=== DISK USAGE ===\n{}\n=== FILE INVENTORY ===\n{}", du_text, find_text);

    // 2. Conductor handshake
    let api_key = std::env::var("GEMINI_API_KEY")
        .expect("ERROR: GEMINI_API_KEY not found in .env");

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}",
        api_key
    );

    let client = Client::new();
    let prompt = format!(
        "As the Nexus Infrastructure Auditor, analyze this build path metadata:\n{}\n\
        Verify that the models in /vault are correctly positioned for ARM64 inference. \
        Check for logical fragmentation or redundant dependencies.",
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
