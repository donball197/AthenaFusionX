// src/main.rs — v3.6 [ARC_STABILIZED]
use nexus_singularity::arbiter_agent::ArbiterAgent;
use nexus_singularity::cli::run_cli_loop;
use nexus_singularity::orchestrator::Orchestrator;
use nexus_singularity::self_healing_engine::SelfHealingEngine;
use nexus_singularity::server::{start_server, AppState, VmoState};
use std::env;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() {
    println!("\x1b[1;36m>> [NEXUS] IGNITION SEQUENCE INITIATED (v3.6.0)\x1b[0m");
    dotenvy::dotenv().ok();

    let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| "UNSET".to_string());
    let (tx, _) = broadcast::channel(256);
    nexus_singularity::logger::init_tracing(tx.clone());

    let memory = Arc::new(Mutex::new(
        nexus_singularity::memory::MemoryPalace::new(".nexus_memory.db")
            .await
            .expect("[MAIN] Fatal: cannot open MemoryPalace SQLite database"),
    ));

    let agent = Arc::new(
        nexus_singularity::neural_agent::MicroAgent::init(tx.clone())
            .expect("[MAIN] Fatal: MicroAgent init failed"),
    );

    let monitor = Arc::new(Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
    let cluster = Arc::new(nexus_singularity::cluster::ClusterMesh::new());
    let governance_log = Arc::new(parking_lot::Mutex::new(nexus_singularity::governance::protocol::GovernanceLog::init()));
    
    let dist = Arc::new(nexus_singularity::distributed_scheduler::DistributedScheduler::new(
        cluster.clone(),
    ));
    
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

    let workspace_root =
        std::env::current_dir().expect("[MAIN] Fatal: cannot resolve current working directory");

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor: monitor.clone(),
        agent: agent.clone(),
        orchestrator: orch,
        memory: memory.clone(),
        api_key: api_key.clone(),
        current_resonance: Arc::new(std::sync::atomic::AtomicU32::new(50)),
        tx: tx.clone(),
        scheduler_tx: scheduler_tx.clone(),
        arbiter: Arc::new(ArbiterAgent::new(agent.clone())),
        workspace_root,
        vmo_state,
        cluster: cluster.clone(),
        dist: dist.clone(),
        governance_log: governance_log.clone(),
    });

    // ── BACKGROUND: HEARTBEAT ──────────────────────────────────────────
    let hb_mesh = cluster.clone();
    tokio::spawn(async move {
        nexus_singularity::heartbeat::start(
            hb_mesh,
            std::env::var("NEXUS_NODE_ID").unwrap_or_else(|_| "DUET_CORE".into()),
            3000,
        )
        .await;
    });

    // ── BACKGROUND: HTTP SERVER ──────────────────────────────────────────
    let s_state = state.clone();
    tokio::spawn(async move {
        start_server(s_state).await;
    });

    // ── BACKGROUND: SELF-HEALING ENGINE ─────────────────────────────────
    let h_state = state.clone();
    let h_memory = h_state.memory.clone();
    tokio::spawn(async move {
        let engine = SelfHealingEngine::new(h_state, h_memory);
        engine.ignite_autonomic_loop().await;
    });

    // ── BACKGROUND: DREAM CYCLE ──────────────────────────────────────────
    let d_state = state.clone();
    tokio::spawn(async move {
        nexus_singularity::dream_cycle::start_daydream(d_state.memory.clone(), d_state.agent.clone(), d_state.api_key.clone()).await;
    });

    // ── BACKGROUND: KNOWLEDGE BROKER & TELEMETRY ─────────────────────────
    let b_state = state.clone();
    tokio::spawn(async move {
        nexus_singularity::broker::KnowledgeBroker::refresh_atlas().await;
        nexus_singularity::telemetry_bridge::start_thermal_bridge(b_state.monitor.clone()).await;
    });

    if let Err(e) = run_cli_loop(state).await {
        eprintln!("!! [FATAL] CLI crashed: {}", e);
    }
}
