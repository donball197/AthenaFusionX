mod hiram_core;
mod vision;
mod models;
mod neural_agent;
mod refactor;
mod auditor;
mod janitor;

use tokio::time::{sleep, Duration};
use reqwest::Client;
use std::env;
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let api_key = env::var("GEMINI_API_KEY")?;
    let client = Client::new();
    let agent = neural_agent::MicroAgent::init()?;
    let refactor_engine = refactor::RefactorEngine::new();
    let mut generation = 0;

    println!("--- NEXUS SINGULARITY: PROTECTED FORGE ---");

    loop {
        // Safety check first
        if !janitor::check_storage_safety() {
            eprintln!(">> [FATAL] STORAGE BREACH! Shutting down.");
            std::process::exit(1);
        }

        println!(">> [FORGE] GENERATION: {}", generation);
        
        // We initialize as mutable to support potential AI evolutions to &mut self
        let mut h = hiram_core::Hiram::new();
        h.verify();

        if let Ok(image_path) = vision::capture_frame().await {
            println!(">> [AGENT 4] Handshaking with Gemma-3-27B-it...");
            if let Ok(resp) = agent.explain_vision(&client, &api_key, &image_path).await {
                let _ = fs::remove_file(&image_path); // Atomic cleanup

                if resp.contains("FIX_CORE:") {
                    let new_code = resp.split("FIX_CORE:").collect::<Vec<&str>>()[1].trim();
                    if let Ok(_) = refactor_engine.apply_patch("src/hiram_core.rs", new_code) {
                        println!(">> [FORGE] Evolution successful.");
                        generation += 1;
                    }
                }
            }
        }
        sleep(Duration::from_secs(30)).await;
    }
}
