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
mod resonator;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use crate::podman::PodmanEngine;
use crate::resonator::ResonatorCore;

fn f32_to_atomic(val: f32) -> u32 { val.to_bits() }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    print!("\x1B[2J\x1B[1;1H");
    println!(">> [NEXUS: SINGULARITY] HYBRID CORE IGNITION...");

    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    if !sentry::is_fit_for_work() { println!(">> [WARNING] CPU LOAD CRITICAL."); }

    let _ = PodmanEngine::ensure_synapse();

    // SHARED ATOMIC RESONANCE (Layer 12)
    let shared_resonance = Arc::new(AtomicU32::new(f32_to_atomic(0.0)));
    let monitor_resonance = shared_resonance.clone();

    tokio::spawn(async move {
        let primary = "vault/gemma3n_e2b.gguf";
        let secondary = "vault/gemma1b.gguf";
        let target = if std::path::Path::new(primary).exists() { primary } else { secondary };

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
    });

    server::start_server(state).await;
}