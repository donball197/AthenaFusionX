use std::process::Command;
use anyhow::{Context, Result};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    println!(">> [LOCAL JANITOR] Booting local inference engine...");

    // The Janitor doesn't call Google. It calls the local Wasm sandbox.
    let output = Command::new("wasmedge")
        .args([
            "--dir", ".:.", 
            "vision_guest/target/wasm32-wasip1/release/vision_guest.wasm",
            "vault/models/tiny_janitor.tflite", // A local, small-footprint model
            "src/main.rs"                      // The target to clean
        ])
        .output()
        .context("Local inference failed. Is WasmEdge installed?")?;

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout);
        println!(">> [LOCAL JANITOR] Task Complete: {}", result);
        
        // CHECK IN WITH CONDUCTOR
        let client = reqwest::Client::new();
        let _ = client.post("http://localhost:3000/checkin")
            .json(&json!({
                "agent": "Janitor",
                "report": "Local cleanup of main.rs finished successfully.",
                "local": true
            }))
            .send()
            .await;
    }

    Ok(())
}