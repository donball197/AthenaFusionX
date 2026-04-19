use anyhow::Result;
use nexus_singularity::{cli, memory, neural_agent, task_manager};
use std::sync::Arc;
use std::io::{self, Write};
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();

    let (tx, _) = broadcast::channel(16);
    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: nexus <directive>");
        return Ok(());
    }

    let raw_directive = args[1..].join(" ");
    let enriched = cli::inject_workspace_dna(&raw_directive);

    println!("\x1b[36m>> [NEXUS] Directive received. Processing...\x1b[0m");

    let mut rx = tx.subscribe();

    let printer = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                // ⚡ SUTURE: Print raw streams live to terminal. 
                // Ignore bulk THOUGHT/EXECUTOR broadcasts since task_manager already println!s them.
                if val["type"].as_str() == Some("NEURAL_STREAM") {
                    if let Some(text) = val["payload"].as_str() {
                        print!("{}", text);
                        io::stdout().flush().ok();
                    }
                }
            }
        }
    });

    let _ = task_manager::send_task(
        agent, &client, &api_key, &enriched, memory, tx
    ).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    printer.abort();
    println!("\n\x1b[32m>> [DONE]\x1b[0m");
    Ok(())
}
