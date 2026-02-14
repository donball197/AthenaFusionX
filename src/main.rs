mod hiram_core;
mod vision;
mod models;
mod neural_agent;
mod refactor;
mod auditor;
mod janitor;
mod context;
mod network;

use tokio::time::{sleep, Duration};
use std::env;
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let api_key = env::var("GEMINI_API_KEY")?;
    
    // Explicitly set to MOTOROLA for this node
    let mode = env::var("NEXUS_MODE").unwrap_or_else(|_| "MOTOROLA".to_string());
    
    let client = reqwest::Client::new();
    let agent = neural_agent::MicroAgent::init()?;
    let refactor_engine = refactor::RefactorEngine::new();

    if mode == "DUET" {
        network::start_duet_listener();
        return Ok(());
    }

    println!(">> [NEXUS] MOTOROLA G PRIMARY ACTIVE. ENGINE: PLUMB.");

    loop {
        // 1. Hardware Safety Check
        if !janitor::check_storage_safety() {
            println!(">> [JANITOR] Storage Critical. Cleaning deck...");
        }

        // 2. Local State Verification
        let mut h = hiram_core::Hiram::new(10);
        h.verify();

        // 3. Vision-Based Evolution
        if let Ok(img) = vision::capture_frame().await {
            println!(">> [EYE] Reality Ingested. Analyzing Delta...");
            
            if let Ok(resp) = agent.evolve_kernel(&client, &api_key, &img).await {
                let _ = fs::remove_file(&img);
                
                if resp.contains("CODE:") {
                    let new_code = resp.split("CODE:").collect::<Vec<&str>>()[1].trim();
                    
                    // 4. Auditor Pre-check (Shadow Build)
                    println!(">> [AUDITOR] Commencing Shadow Audit...");
                    if auditor::validate_patch("src/shadow_core.rs", new_code) {
                        refactor_engine.apply_patch("src/hiram_core.rs", new_code)?;
                        println!(">> [KERNEL] Evolution Seated. Logic is Plumb.");
                    } else {
                        println!(">> [AUDITOR] Hallucination Rejected. Stability Maintained.");
                    }
                }
            }
        }
        
        println!(">> [FORGE] Cooling loop [45s]...");
        sleep(Duration::from_secs(45)).await;
    }
}
