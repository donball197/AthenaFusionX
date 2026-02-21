mod server;
mod neural_agent;
mod memory;
mod system_monitor;
mod vision;
mod podman;
mod polyglot;
mod hive_mind;
mod auditor;
mod janitor;
mod sentry;
mod resonator; // Zig Bridge

use std::sync::Arc;
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use crate::podman::PodmanEngine;
use crate::resonator::ResonatorCore;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    print!("\x1B[2J\x1B[1;1H");
    println!(">> [NEXUS: SINGULARITY] SYSTEM IGNITION...");

    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    
    // 1. Safety Check
    if !sentry::is_fit_for_work() {
        println!(">> [WARNING] SYSTEM LOAD HIGH. PROCEED WITH CAUTION.");
    }

    // 2. Load Vault (Zig/TFLite)
    // We check specifically for the vision model you listed in 'find .'
    match ResonatorCore::ignite_vault("./vault/vision_v2.tflite") {
        Ok(msg) => println!(">> [VAULT] Vision Cortex: {}", msg),
        Err(_) => println!(">> [VAULT] Vision Cortex: OFFLINE (Model not in ./vault)"),
    }

    // 3. Connect Infrastructure
    match PodmanEngine::ensure_synapse() {
        Ok(_) => println!(">> [INFRA] Synapse: ONLINE"),
        Err(e) => println!(">> [INFRA] Synapse FAIL: {}", e),
    }

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(),
        agent: MicroAgent::init().expect("Failed to initialize MicroAgent"),
        memory: MemoryPalace::new(),
        api_key,
        hive: crate::hive_mind::HiveMind::connect().ok(),
    });

    server::start_server(state).await;
}
