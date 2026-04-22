// src/bin/cli.rs — v4.1 NEXUS DevOps CLI Binary
// Copyright 2026 Donald Ball — Apache 2.0
//
// FIX: `agent` variable was created but never directly used.
//      Renamed to `_agent` — the init side-effect (metric registration,
//      allocator warm-up) is preserved, warning is gone.

use anyhow::Result;
use nexus_singularity::{
    cli,
    memory,
    neural_agent,
    task_manager,
    server::VmoState,
    orchestrator::Orchestrator,
};
use std::sync::Arc;
use std::io::{self, Write};
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();

    let (tx, _) = broadcast::channel(16);

    // _agent: init side-effect only — warms allocator and registers metrics.
    let _agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);

    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory        = Arc::new(Mutex::new(memory_palace));
    let vmo_state     = Arc::new(Mutex::new(VmoState::default()));
    let orchestrator  = Arc::new(Orchestrator::new());

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: nexus <directive>");
        return Ok(());
    }

    let raw_directive     = args[1..].join(" ");
    let enriched_directive = cli::inject_workspace_dna(&raw_directive);

    println!("\x1b[36m>> [CLI_UPLINK] Directive → NEXUS Cortex (gemma-4-26b-a4b-it primary)\x1b[0m");

    let mut rx = tx.subscribe();

    let printer = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                match val["type"].as_str().unwrap_or("") {
                    "NEURAL_STREAM" => {
                        if let Some(text) = val["payload"].as_str() {
                            print!("{}", text);
                            io::stdout().flush().ok();
                        }
                    }
                    "NEURAL_THOUGHT" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\x1b[34m[THOUGHT] {}\x1b[0m", text);
                        }
                    }
                    "TASK_STATUS" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\n\x1b[32m>> [STATUS] {}\x1b[0m", text);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    let _ = task_manager::send_task(
        orchestrator,
        &client,
        &api_key,
        &enriched_directive,
        memory,
        vmo_state,
        tx,
    )
    .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    printer.abort();

    println!("\n\x1b[32m>> [DONE]\x1b[0m");
    Ok(())
}
