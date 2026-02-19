mod hiram_core;
mod neural_agent;
mod auditor;
mod broker;
mod janitor;
mod monitor;
mod alloc;
mod sentry;
mod server;
mod memory;
mod brain;
mod system_monitor;
mod vision;
mod calibrate;
mod network;

use tokio::time::{sleep, Duration};
use std::{env, sync::Arc};
use system_monitor::SystemMonitor;
use neural_agent::MicroAgent;
use memory::MemoryPalace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    
    print!("\x1B[2J\x1B[1;1H");
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "NEXUS_DUET".to_string());
    let api_key = env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set in .env");

    println!(">> [NEXUS ENGINE: SUPREME] NODE [{}] ONLINE. MODEL: GEMMA-3-27B-IT", node_id);

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(),
        agent: MicroAgent::init()?,
        memory: MemoryPalace::new(),
        api_key,
    });

    let server_state = state.clone();
    tokio::spawn(async move {
        // FIXED: We just await the server. If it fails, it panics internally.
        server::start_server(server_state).await;
    });

    println!(">> [NEXUS] ALL SYSTEMS GREEN.");
    loop { sleep(Duration::from_secs(60)).await; }
}