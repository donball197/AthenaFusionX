// src/main.rs
#![allow(dead_code, unused_variables)]

pub mod inquisitor;
pub mod watcher;
pub mod auto_healer;
pub mod server;
pub mod neural_agent;
pub mod memory;
pub mod system_monitor;
pub mod vision;
pub mod podman;
pub mod polyglot;
pub mod hive_mind;
pub mod auditor;
pub mod broker;
pub mod janitor;
pub mod sentry;
pub mod resonator;
pub mod industrial;
pub mod devops;
pub mod hiram_core;
pub mod tflite_bridge;

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use crate::resonator::ResonatorCore;
use sysinfo::System;

fn f32_to_atomic(val: f32) -> u32 { val.to_bits() }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    println!(">> [NEXUS-DEV] IGNITING HYBRID KERNEL v0.9.9...");

    // 1. API KEY HANDSHAKE
    let api_key = std::env::var("GEMINI_API_KEY").expect("CRITICAL: API_KEY missing");

    // 2. ZIG HARDWARE ANCHOR
    let vault_model = "vault/models/gemma3n_e2b.gguf";
    if std::path::Path::new(vault_model).exists() {
        match ResonatorCore::ignite_vault(vault_model) {
            Ok(msg) => println!(">> [VAULT] Hardware Locked: {}", msg),
            Err(_) => println!(">> [VAULT] Fallback: Cloud-Only Mode."),
        }
    }

    let shared_resonance = Arc::new(std::sync::atomic::AtomicU32::new(f32_to_atomic(1.0)));
    let monitor_resonance = shared_resonance.clone();
    let model_path = vault_model.to_string();

    // 3. RESONANCE HEARTBEAT
    tokio::spawn(async move {
        loop {
            if let Ok(energy) = ResonatorCore::scan_local_energy(&model_path) {
                monitor_resonance.store(f32_to_atomic(energy), std::sync::atomic::Ordering::Relaxed);
            }
            sleep(Duration::from_secs(10)).await;
        }
    });

    let sys = Arc::new(std::sync::Mutex::new(System::new_all()));
    let memory = Arc::new(Mutex::new(MemoryPalace::new()));

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemini-2.5-flash-lite".to_string(),
        monitor: SystemMonitor::new(sys.clone()),
        agent: MicroAgent::init().expect("Agent init fail"),
        memory: memory.clone(),
        api_key,
        hive: None,
        current_resonance: shared_resonance,
        hiram: Arc::new(Mutex::new(crate::hiram_core::Hiram::new(3600))),
        last_health_error: Arc::new(Mutex::new(None)),
    });

    // 4. START JANITOR DREAM CYCLE
    let janitor_state = state.clone();
    tokio::spawn(crate::janitor::Janitor::start_background_maintenance(janitor_state));

    // 5. LAUNCH
    println!(">> [HIVE QUEEN] Control interface online.");
    server::start_server(state).await;
}
