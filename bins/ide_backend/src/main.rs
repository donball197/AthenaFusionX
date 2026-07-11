#![allow(dead_code, unused_must_use, unused_imports, unused_variables)]
mod auth; mod broker; mod cluster_api; mod dream_cycle; mod edge_worker;
mod fs_watcher; mod gateway_trait; mod heartbeat; mod logger; mod scheduler;
mod security; mod server; mod server_terminal; mod system_monitor;
mod telemetry_bridge; mod thermal_governor; mod vitals_handler; mod ws_bridge;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use secrecy::SecretString;
use std::path::PathBuf;

use nexus_substrate::memory::MemoryPalace;
use nexus_runtime_core::shared_state::VmoState;
use nexus_runtime_core::cluster::ClusterMesh;
use nexus_runtime_core::distributed_scheduler::DistributedScheduler;
use nexus_runtime_core::orchestrator::Orchestrator;
use nexus_model_hub::neural_agent::MicroAgent;
use nexus_runtime_core::arbiter_agent::ArbiterAgent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logger::init();
    tracing::info!(">> [BOOT] NEXUS IDE BACKEND INITIALIZING...");

    if let Err(e) = security::write_startup_token() {
        tracing::warn!("Could not write UI token (ensure NEXUS_AUTH_KEY is >=32 bytes in .env): {}", e);
    }

    let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_else(|_| "DUMMY_KEY".to_string());
    let (tx, _) = broadcast::channel::<String>(256);
    let memory = Arc::new(Mutex::new(MemoryPalace::new("nexus_backend.db").await?));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
    let cluster = Arc::new(ClusterMesh::new());
    let dist = Arc::new(DistributedScheduler::new(cluster.clone()));
    let orchestrator = Arc::new(Orchestrator::new());
    let client = reqwest::Client::new();
    
    let agent = Arc::new(MicroAgent::init(tx.clone())?);
    let arbiter = Arc::new(ArbiterAgent::new(agent.clone()));
    let monitor = Arc::new(Mutex::new(system_monitor::SystemMonitor::new()));

    let state = Arc::new(server::AppState {
        client, monitor: monitor.clone(), agent, orchestrator, memory,
        api_key: SecretString::new(api_key.into_boxed_str()), tx: tx.clone(), arbiter,
        workspace_root: PathBuf::from("."), vmo_state, cluster, dist,
    });

    heartbeat::spawn(tx.clone());
    telemetry_bridge::spawn(tx.clone(), monitor, Arc::new(std::sync::atomic::AtomicU32::new(1000)));
    thermal_governor::spawn();

    tracing::info!("⚡ COGNITIVE KERNEL SERVICE ONLINE at 0.0.0.0:3000");
    server::start_server(state).await;
    Ok(())
}
