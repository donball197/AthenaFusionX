use nexus_singularity::{
    arbiter_agent::ArbiterAgent,
    cli::run_cli_loop,
    orchestrator::Orchestrator,
    security,
    self_healing_engine::SelfHealingEngine,
    server::{start_server, AppState, VmoState},
};
use nexus_substrate::memory::MemoryPalace;
use std::{env, sync::Arc};
use tokio::sync::{broadcast, Mutex};

// Nexus Singularity v1.9.0 - Builder Test Passed
// Nexus Singularity v1.9.0 - Builder Test Passed
// Nexus Singularity v1.9.0 - Builder Test Passed
// Nexus Singularity v1.9.0 - Builder Test Passed
// Nexus Singularity v1.9.0 - Builder Test Passed
// Nexus Singularity v1.9.0 - Builder Test Passed
// Nexus Singularity v1.9.0 - Builder Test Passed
#[tokio::main]
async fn main() {
    // ⚡ FIX: Install default crypto provider for jsonwebtoken v10

    println!(">> [NEXUS] IGNITION SEQUENCE INITIATED (v3.9.2)");
    dotenvy::dotenv().ok();

    let api_key = env::var("GEMINI_API_KEY")
        .expect("[MAIN] Fatal: GEMINI_API_KEY not found in .env file.");

    let (tx, _) = broadcast::channel(256);
    nexus_singularity::logger::init_tracing(tx.clone());

    let _watcher = nexus_singularity::fs_watcher::NexusWatcher::ignite(tx.clone())
        .expect("[REPAIR] Failed to ignite FS_WATCHER");

    if let Err(e) = security::write_startup_token() {
        eprintln!(
            "\x1b[31m!! [SECURITY] Could not write startup token: {}.              Ensure NEXUS_AUTH_KEY (>= 32 bytes) is set.\x1b[0m", e
        );
    }

    let memory = Arc::new(Mutex::new(
        MemoryPalace::new(".nexus_memory.db")
            .await
            .expect("[MAIN] Fatal: cannot open MemoryPalace SQLite"),
    ));

    let agent = Arc::new(
        nexus_singularity::neural_agent::MicroAgent::init(tx.clone())
            .expect("[MAIN] Fatal: MicroAgent init failed"),
    );

    let monitor        = Arc::new(Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()));
    let vmo_state      = Arc::new(Mutex::new(VmoState::default()));
    let cluster        = Arc::new(nexus_singularity::cluster::ClusterMesh::new());
    let governance_log = Arc::new(parking_lot::Mutex::new(
        nexus_singularity::governance::protocol::GovernanceLog::init(),
    ));
    let dist = Arc::new(nexus_singularity::distributed_scheduler::DistributedScheduler::new(cluster.clone()));
    let orch = Arc::new(Orchestrator::new());

    let scheduler_tx = nexus_singularity::scheduler::MasterSupervisor::ignite(
        tx.clone(),
        reqwest::Client::new(),
        api_key.clone(),
        orch.clone(),
        Arc::new(std::sync::atomic::AtomicU32::new(50)),
        nexus_singularity::system_monitor::SystemMonitor::new(),
        memory.clone(),
        vmo_state.clone(),
        dist.clone(),
    );

    let workspace_root = env::current_dir()
        .expect("[MAIN] Fatal: cannot resolve working directory");

    let state = Arc::new(AppState {
        client:            reqwest::Client::new(),
        monitor:           monitor.clone(),
        agent:             agent.clone(),
        orchestrator:      orch,
        memory:            memory.clone(),
        api_key:           api_key.clone(),
        current_resonance: Arc::new(std::sync::atomic::AtomicU32::new(50)),
        tx:                tx.clone(),
        scheduler_tx,
        arbiter:           Arc::new(ArbiterAgent::new(agent.clone())),
        workspace_root,
        vmo_state,
        cluster:           cluster.clone(),
        dist:              dist.clone(),
        governance_log,
    });

    // Subsystems
    let hb_mesh = cluster.clone();
    tokio::spawn(async move {
        nexus_singularity::heartbeat::start(
            hb_mesh,
            env::var("NEXUS_NODE_ID").unwrap_or_else(|_| "DUET_CORE".into()),
            3000,
        ).await;
    });

    let s_state = state.clone();
    tokio::spawn(async move { start_server(s_state).await; });

    let h_state  = state.clone();
    let h_memory = memory.clone();
    tokio::spawn(async move {
        SelfHealingEngine::new(h_state, h_memory).ignite_autonomic_loop().await;
    });

    let d_memory = memory.clone();
    let d_agent = agent.clone();
    let d_api_key = api_key.clone();
    tokio::spawn(async move {
        nexus_singularity::dream_cycle::start_daydream(
            d_memory,
            d_agent,
            d_api_key,
        ).await;
    });

    let b_monitor = monitor.clone();
    tokio::spawn(async move {
        nexus_singularity::broker::KnowledgeBroker::refresh_atlas().await;
        nexus_singularity::telemetry_bridge::start_thermal_bridge(b_monitor).await;
    });

    if let Err(e) = run_cli_loop(state).await {
        eprintln!("!! [FATAL] CLI crashed: {}", e);
    }
}