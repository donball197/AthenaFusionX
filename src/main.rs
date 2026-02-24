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

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use sysinfo::System;
use prometheus::{Gauge, Opts};

fn f32_to_atomic(val: f32) -> u32 { val.to_bits() }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    println!(">> [NEXUS] IGNITING GLOBAL OBSERVABILITY...");

    let healer_opts = Opts::new("healer_status", "Nexus Health Status");
    let healer_gauge = Gauge::with_opts(healer_opts).unwrap();
    let _ = prometheus::register(Box::new(healer_gauge.clone()));

    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    let shared_resonance = Arc::new(std::sync::atomic::AtomicU32::new(f32_to_atomic(1.0)));

    let sys = Arc::new(std::sync::Mutex::new(System::new_all()));
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(sys.clone()),
        agent: MicroAgent::init().expect("Failed to initialize MicroAgent"),
        memory: MemoryPalace::new(),
        api_key,
        hive: None,
        current_resonance: shared_resonance,
        hiram: Arc::new(tokio::sync::Mutex::new(crate::hiram_core::Hiram::new(60))),
        last_health_error: Arc::new(tokio::sync::Mutex::new(None)),
    });

    let hiram_arc = state.hiram.clone();

    tokio::spawn(async move {
        loop {
            let diag = crate::auto_healer::AutoHealer::run_diagnostic().await;
            if diag.is_none() { healer_gauge.set(1.0); } else { healer_gauge.set(0.0); }
            if let Ok(mut hiram) = hiram_arc.try_lock() {
                hiram.verify();
            }
            sleep(Duration::from_secs(10)).await;
        }
    });

    println!(">> [NEXUS] SYSTEMS GREEN. LISTENING ON PORT 3000.");
    server::start_server(state).await;
}