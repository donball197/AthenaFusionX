mod auditor;
mod podman;
mod polyglot;
mod hiram_core;
mod neural_agent;
mod server;
mod memory;
mod system_monitor;
mod vision;
mod network;
mod janitor;
mod broker;
mod sentry;
mod alloc;
mod brain;
mod calibrate;

use tokio::time::{sleep, Duration};
use std::{env, sync::Arc};
use system_monitor::SystemMonitor;
use neural_agent::MicroAgent;
use memory::MemoryPalace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    print!("\x1B[2J\x1B[1;1H");
    
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "NEXUS_ARM64".to_string());
    let api_key = env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    
    println!(">> [NEXUS: DEVOPS OS] NODE [{}] ONLINE. ARCH: ARM64", node_id);
    println!(">> [INFRA] Podman CLI: LINKED");
    
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
        server::start_server(server_state).await;
    });
    
    println!("[NEXUS] SYSTEMS GREEN. WAITING FOR COMMAND.");
    loop { sleep(Duration::from_secs(60)).await; }
}