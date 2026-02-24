#![allow(dead_code)]
mod server;
mod neural_agent;
mod memory;
mod system_monitor;
mod vision;
mod podman;
mod polyglot;
mod hive_mind;
mod auditor;
mod broker;
mod janitor;
mod sentry;
mod resonator;
mod industrial;
mod devops;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use crate::resonator::ResonatorCore;
use crate::podman::PodmanEngine;

fn f32_to_atomic(val: f32) -> u32 { val.to_bits() }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    print!("\x1B[2J\x1B[1;1H");
    println!(">> [NEXUS: SINGULARITY] HYBRID CORE IGNITION...");
    
    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    
    // Safety check
    if !sentry::is_fit_for_work() { println!(">> [WARNING] CPU LOAD CRITICAL."); }

    // Init Infra
    match PodmanEngine::ensure_synapse() {
        Ok(_) => println!(">> [INFRA] Synapse: ONLINE"),
        Err(_) => println!(">> [INFRA] Synapse FAIL (Is Podman running?)"),
    }

    let shared_resonance = Arc::new(AtomicU32::new(f32_to_atomic(0.0)));
    let monitor_resonance = shared_resonance.clone();

    // Resonance Pulse Thread
    tokio::spawn(async move {
        let target = "vault/gemma3n_e2b.gguf";
        loop {
            if std::path::Path::new(target).exists() {
                if let Ok(energy) = ResonatorCore::scan_local_energy(target) {
                    monitor_resonance.store(f32_to_atomic(energy), Ordering::Relaxed);
                }
            }
            sleep(Duration::from_secs(10)).await;
        }
    });

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(),
        agent: MicroAgent::init().expect("Failed to initialize MicroAgent"),
        memory: MemoryPalace::new(),
        api_key,
        hive: crate::hive_mind::HiveMind::connect().ok(),
        current_resonance: shared_resonance,
        last_health_error: Arc::new(tokio::sync::Mutex::new(None)),
    });

    server::start_server(state).await;
}