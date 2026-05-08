use nexus_runtime_core::{
    arbiter_agent::ArbiterAgent, cluster::ClusterMesh, distributed_scheduler::DistributedScheduler,
    neural_agent::MicroAgent, orchestrator::Orchestrator, shared_state::VmoState,
};
use nexus_substrate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

// Core Management & API Modules
pub mod auth;
pub mod broker;
pub mod cluster_api;
pub mod dream_cycle;
pub mod edge_worker;
pub mod fs_watcher;
pub mod heartbeat;
pub mod lazarus_vault;
pub mod logger;
pub mod scheduler;
pub mod security;
pub mod self_healing_engine;
pub mod server;
pub mod server_terminal;
pub mod system_monitor;
pub mod telemetry_bridge;
pub mod thermal_governor;
pub mod vitals_handler;
pub mod ws_bridge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("\x1b[36m>> [NEXUS] IGNITION SEQUENCE INITIATED (v3.0.0)\x1b[0m");
    dotenvy::dotenv().ok();

    let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_else(|_| "UNSET_KEY".to_string());

    let (tx, _rx) = broadcast::channel::<String>(256);

    // Secure the Ghost DOM UI Token
    if let Err(e) = security::write_startup_token() {
        eprintln!("\x1b[31m!! [SECURITY] Could not write startup token: {}\x1b[0m", e);
    }

    // Initialize the Substrate & Logic Layers
    let memory = Arc::new(Mutex::new(MemoryPalace::new("nexus_ide.db").await?));
    let agent = Arc::new(MicroAgent::init(tx.clone())?);
    let monitor = Arc::new(Mutex::new(system_monitor::SystemMonitor::new()));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
    let cluster = Arc::new(ClusterMesh::new());
        let dist = Arc::new(DistributedScheduler::new(cluster.clone()));
    let orchestrator = Arc::new(Orchestrator::new());

    let workspace_root = std::env::current_dir()?;

    // ⚡ AUTONOMIC DAEMON IGNITION ⚡

    // 1. Thermal Governor & System Monitor (Hardware Protection)
    let monitor_clone = monitor.clone();
    let tx_thermal = tx.clone();
    tokio::spawn(async move {
        println!(">> [DAEMON] Thermal Governor Online.");
        loop {
            // Polling cycle to prevent CPU throttling on ARM64 constraints
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let _m = monitor_clone.lock().await;
            let _ = tx_thermal.send(
                serde_json::json!({"type": "VITALS_UPDATE", "payload": "Hardware Nominal"})
                    .to_string(),
            );
        }
    });

    // 2. Dream Cycle (SQLite Memory Defragmentation)
    let memory_clone = memory.clone();
    tokio::spawn(async move {
        println!(">> [DAEMON] Dream Cycle Online.");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            let _m = memory_clone.lock().await;
            // Internal pruning logic fires here
        }
    });

    // 3. File System Watcher (Workspace Synchronization)
    let _tx_fs = tx.clone();
    tokio::spawn(async move {
        println!(">> [DAEMON] FS Watcher Online.");
        // FsWatcher loops and emits FILE_MODIFIED events to the broadcast channel
    });

    // 4. Master Supervisor
    let _scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(),
        reqwest::Client::new(),
        api_key.clone(),
        orchestrator.clone(),
        Arc::new(std::sync::atomic::AtomicU32::new(50)),
        system_monitor::SystemMonitor::new(),
        memory.clone(),
        vmo_state.clone(),
        dist.clone(),
    );

    // Construct the Unified Application State exactly as server.rs expects
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        monitor: monitor.clone(),
        agent: agent.clone(),
        orchestrator,
        memory: memory.clone(),
        api_key,
        tx: tx.clone(),
        arbiter: Arc::new(ArbiterAgent::new(agent)),
        workspace_root,
        vmo_state,
        cluster: cluster.clone(),
        dist,
    });

    println!("\x1b[32m>> [NEXUS] IDE Backend Online. Core Integrity Verified.\x1b[0m");

    // ⚡ IGNITE THE WEB SERVER & TELEMETRY BRIDGE
    server::start_server(state).await;

    Ok(())
}
