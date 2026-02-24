#![allow(dead_code, unused_variables)]
pub mod auto_healer;
mod server;
mod neural_agent;
mod memory;
pub mod system_monitor;
mod vision;
mod podman;
mod polyglot;
mod hive_mind;
mod auditor;
mod broker;
pub mod janitor;
mod sentry;
mod resonator;
mod industrial;
mod devops;
mod hiram_core;
mod mission_control;

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use crate::resonator::ResonatorCore;
use crate::podman::PodmanEngine;
use crate::industrial::IndustrialDrone;
use crate::devops::DevOpsDrone;
use sysinfo::System;

fn f32_to_atomic(val: f32) -> u32 { val.to_bits() }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    print!("\x1B[2J\x1B[1;1H");
    println!(">> [NEXUS: SINGULARITY] HYBRID CORE IGNITION...");
    
    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    
    // 1. ZIG BRIDGE CHECK
    if std::path::Path::new("vault/gemma3n_e2b.gguf").exists() {
        match ResonatorCore::ignite_vault("vault/gemma3n_e2b.gguf") {
            Ok(msg) => println!(">> [VAULT] Zig Resonator: ONLINE ({})", msg),
            Err(_) => println!(">> [VAULT] Zig Bridge: Failed to bind."),
        }
    } else {
        println!(">> [VAULT] No GGUF found. Running Cloud-Only Mode.");
    }

    // 2. PODMAN CHECK
    let _ = PodmanEngine::ensure_synapse();

    // 3. ATOMIC RESONANCE
    let shared_resonance = Arc::new(std::sync::atomic::AtomicU32::new(f32_to_atomic(0.0)));
    let monitor_resonance = shared_resonance.clone();

    // Background Thread: Zig Entropy Scan
    tokio::spawn(async move {
        let target = "vault/gemma3n_e2b.gguf";
        loop {
            if std::path::Path::new(target).exists() {
                if let Ok(energy) = ResonatorCore::scan_local_energy(target) {
                    monitor_resonance.store(f32_to_atomic(energy), std::sync::atomic::Ordering::Relaxed);
                }
            }
            sleep(Duration::from_secs(10)).await;
        }
    });

    let sys = Arc::new(std::sync::Mutex::new(System::new_all()));
    
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(sys.clone()),
        agent: MicroAgent::init().expect("Failed to initialize MicroAgent"),
        memory: MemoryPalace::new(),

      
        api_key,
        hive: crate::hive_mind::HiveMind::connect().ok(),
        current_resonance: shared_resonance,
        hiram: Arc::new(tokio::sync::Mutex::new(crate::hiram_core::Hiram::new(60))),
        last_health_error: Arc::new(tokio::sync::Mutex::new(None)),
    });

    let healer_state = state.last_health_error.clone();
    let hiram_arc = state.hiram.clone();
    
    // 4. AUTONOMOUS WATCHDOGS
    tokio::spawn(async move {
        loop {
            let diag = crate::auto_healer::AutoHealer::run_diagnostic().await;
            {
                let mut hiram = hiram_arc.lock().await;
                hiram.verify();
            }
            DevOpsDrone::run_background_check(".", "nexus-core");
            let _ = IndustrialDrone::auto_throttle().await;

            *healer_state.lock().await = diag;
            sleep(Duration::from_secs(10)).await;
        }
    });

    println!(">> [NEXUS] SYSTEMS GREEN. LISTENING ON PORT 3000.");
    server::start_server(state).await;
}
