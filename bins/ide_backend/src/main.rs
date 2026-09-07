#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use nexus_runtime_core::cluster::ClusterMesh;
use nexus_runtime_core::distributed_scheduler::DistributedScheduler;
use nexus_runtime_core::mcp::McpServer;
use nexus_runtime_core::memory_cortex::MemoryCortex;
use nexus_runtime_core::orchestrator::Orchestrator;
use nexus_runtime_core::shared_state::VmoState;
use nexus_substrate::memory::MemoryPalace;
use secrecy::SecretString;

mod auth;
mod broker;
mod cluster_api;
mod dream_cycle;
mod edge_worker;
mod fs_watcher;
mod gateway_trait;
mod heartbeat;
mod logger;
mod scheduler;
mod security;
mod self_healing_engine;
mod server;
mod server_terminal;
mod telemetry_bridge;
mod thermal_governor;
mod vitals_handler;
mod ws_bridge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logger::init();

    tracing::info!("==================================================");
    tracing::info!(">> NEXUS SINGULARITY v4.0 — DAEMON ONLINE");
    tracing::info!("==================================================");

    security::write_startup_token()?;
    let api_key_str = std::env::var("GEMINI_API_KEY").unwrap_or_default().trim().to_string();
    let api_key = SecretString::from(api_key_str.clone());
    let req_client = reqwest::Client::new();

    let (tx, _) = broadcast::channel::<String>(256);

    let palace = Arc::new(MemoryPalace::new("nexus_daemon.db").await?);

    // 👇 The crucial fix: We clone the palace Arc so it can be shared!
    let memory = Arc::new(MemoryCortex::new(palace.clone()));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
    let cluster = Arc::new(ClusterMesh::new());
    let dist = Arc::new(DistributedScheduler::new(cluster.clone()));
    let orchestrator = Arc::new(Orchestrator::new());

    let monitor = Arc::new(Mutex::new(nexus_ide_backend::system_monitor::SystemMonitor::new()));
    let resonance = Arc::new(std::sync::atomic::AtomicU32::new(1000));

    let agent = Arc::new(nexus_model_hub::neural_agent::MicroAgent::init(tx.clone())?);
    let arbiter = Arc::new(nexus_runtime_core::arbiter_agent::ArbiterAgent::new(agent.clone()));

    // 🌟 Boot all Subconscious Background Threads 🌟
    dream_cycle::spawn(
        palace.clone(),
        tx.clone(),
        req_client.clone(),
        api_key_str.clone(),
        orchestrator.clone(),
    );
    self_healing_engine::spawn(
        orchestrator.clone(),
        req_client.clone(),
        api_key_str.clone(),
        memory.clone(),
        vmo_state.clone(),
        tx.clone(),
        dist.clone(),
        monitor.clone(),
    );
    telemetry_bridge::spawn(tx.clone(), monitor.clone(), resonance.clone());
    thermal_governor::spawn();
    heartbeat::spawn(tx.clone());

    let state = Arc::new(server::AppState {
        client: req_client,
        monitor,
        agent,
        orchestrator,
        memory,
        api_key,
        tx,
        arbiter,
        workspace_root: PathBuf::from("."),
        vmo_state,
        cluster,
        dist,
        mcp: Arc::new(McpServer::new()),
    });

    // 🌟 Boot WASM Sidecars 🌟
    let sidecars_dir = PathBuf::from("sidecars");
    if let Ok(entries) = std::fs::read_dir(&sidecars_dir) {
        if let Ok(sidecar_host) = nexus_runtime_core::sidecar_host::SidecarHost::new(true) {
            let sidecar_host = Arc::new(sidecar_host);
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    tracing::info!(">> [SIDECAR] Loading external cognitive module: {:?}", path);
                    let host = sidecar_host.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = host.load_and_run(&path) {
                            let err_str = e.to_string();
                            if err_str.contains("_start")
                                || err_str.contains("failed to find function export")
                            {
                                tracing::debug!(
                                    "[SIDECAR] Loaded library module {:?} (no _start export)",
                                    path
                                );
                            } else {
                                tracing::error!("[SIDECAR] Failed to execute {:?}: {}", path, e);
                            }
                        }
                    });
                }
            }
        }
    }

    tracing::info!(">> Core Subsystems online. Binding Axum API Gateway to 0.0.0.0:3000...");
    server::start_server(state).await;

    Ok(())
}
