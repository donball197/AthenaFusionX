use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use nexus_singularity::{neural_agent, memory, task_manager, cli};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();
    
    // SUTURE: Binary needs its own broadcast channel for streaming
    let (tx, _) = broadcast::channel(16);
    
    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    // Simple CLI logic for the standalone bin
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: nexus <directive>");
        return Ok(());
    }

    let raw_directive = args[1..].join(" ");
    let enriched_directive = cli::inject_workspace_dna(&raw_directive);

    println!("\x1b[36m>> [CLI_UPLINK] Sending directive to NEXUS Cortex...\x1b[0m");

    // SUTURE: Added the 6th argument (tx)
    let _ = task_manager::send_task(
        agent, 
        &client, 
        &api_key, 
        &enriched_directive, 
        memory, 
        tx
    ).await;

    Ok(())
}
