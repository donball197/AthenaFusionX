// --- NEXUS CORE INTEGRATION ---
mod hiram_core;
mod neural_agent;
mod refactor;
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

#[allow(unused_variables, unused_imports, dead_code)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    
    // ARM64 Optimization
    alloc::initialize_mnemnos_lean();
    
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "NEXUS_DUET".to_string());
    
    // 1. VERIFY CREDENTIALS
    let api_key = env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set in .env");

    println!(">> [FORGE] NODE [{}] ONLINE. MODEL: GEMMA-3-27B-IT", node_id);

    // 2. INJECT NEURAL LINK
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(),
        agent: MicroAgent::init()?, 
        api_key, 
    });

    // 3. ACTIVATE HUD BRIDGE
    let server_state = state.clone();
    tokio::spawn(async move {
        server::start_server(server_state).await;
    });

    println!(">> [NEXUS] Core Loop Active. Neural Link: STABLE.");

    loop {
        sleep(Duration::from_secs(30)).await;
    }
}
