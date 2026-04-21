// src/main.rs — v3.3 [AUTONOMIC_IMMUNE_ACTIVE]
use nexus_singularity::server::{AppState, start_server, VmoState};
use nexus_singularity::cli::run_cli_loop;
use nexus_singularity::orchestrator::Orchestrator;
use nexus_singularity::arbiter_agent::ArbiterAgent;
use nexus_singularity::self_healing_engine::SelfHealingEngine;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use std::env;

#[tokio::main]
async fn main() {
    println!("\x1b[1;36m>> [NEXUS] IGNITION SEQUENCE INITIATED (v3.3.0)\x1b[0m");
    dotenvy::dotenv().ok();

    nexus_singularity::metrics::init_metrics();

    let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| "UNSET".to_string());
    let (tx, _) = broadcast::channel(256);

    let memory = Arc::new(Mutex::new(
        nexus_singularity::memory::MemoryPalace::new(".nexus_memory.db")
            .await
            .unwrap(),
    ));

    let agent = Arc::new(
        nexus_singularity::neural_agent::MicroAgent::init(tx.clone()).unwrap(),
    );

    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
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
    );

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor: Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()),
        agent: agent.clone(),
        orchestrator: orch,
        memory: memory.clone(),
        api_key: api_key.clone(),
        current_resonance: Arc::new(std::sync::atomic::AtomicU32::new(50)),
        tx: tx.clone(),
        scheduler_tx: scheduler_tx.clone(),
        arbiter: Arc::new(ArbiterAgent::new(agent.clone())),
        workspace_root: std::env::current_dir().unwrap_or_default(),
        vmo_state,
    });

    // ── BACKGROUND: HTTP SERVER ──
    let s_state = state.clone();
    tokio::spawn(async move { start_server(s_state).await; });

    // ── BACKGROUND: SELF-HEALING ENGINE ──
    let h_state = state.clone();
    let h_memory = h_state.memory.clone();
    tokio::spawn(async move {
        let engine = SelfHealingEngine::new(h_state, h_memory);
        engine.ignite_autonomic_loop().await;
    });

    // ── BACKGROUND: DREAM CYCLE ──
    let dream_memory = state.memory.clone();
    let dream_agent = state.agent.clone();
    let dream_key = api_key.clone();
    tokio::spawn(async move {
        nexus_singularity::dream_cycle::start_daydream(dream_memory, dream_agent, dream_key).await;
    });

    // ── BACKGROUND: KNOWLEDGE BROKER ──
    tokio::spawn(async move {
        nexus_singularity::broker::KnowledgeBroker::refresh_atlas().await;
        println!("\x1b[34m>> [BROKER] Project atlas indexed.\x1b[0m");
    });

    if let Err(e) = run_cli_loop(state).await {
        eprintln!("!! [FATAL] CLI crashed: {}", e);
    }
}
