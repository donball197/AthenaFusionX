// src/main.rs — v3.1
// UPGRADES over v2.3.1:
//   ADDED: dream_cycle::start_daydream spawned as a background task
//   ADDED: KnowledgeBroker::refresh_atlas called on startup
//   ADDED: Metrics initialisation on boot

use nexus_singularity::server::{AppState, start_server, VmoState};
use nexus_singularity::cli::run_cli_loop;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use std::env;

#[tokio::main]
async fn main() {
    println!("\x1b[1;36m>> [NEXUS] IGNITION SEQUENCE INITIATED (v3.1.0)\x1b[0m");
    dotenvy::dotenv().ok();

    // ── METRICS INIT ─────────────────────────────────────────────────────────
    nexus_singularity::metrics::init_metrics();

    let api_key =
        env::var("GEMINI_API_KEY").unwrap_or_else(|_| "UNSET".to_string());
    let (tx, _) = broadcast::channel(256);

    let memory = Arc::new(Mutex::new(
        nexus_singularity::memory::MemoryPalace::new(".nexus_memory.db")
            .await
            .unwrap(),
    ));

    let agent = Arc::new(
        nexus_singularity::neural_agent::MicroAgent::init(tx.clone()).unwrap(),
    );

    // VmoState resets every boot — no stale handles from prior sessions.
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));

    // ── SWARM ORCHESTRATOR ───────────────────────────────────────────────────
    let orch = Arc::new(nexus_singularity::orchestrator::Orchestrator::new(
        agent.clone(),
        Some(Arc::new(
            nexus_singularity::swarm::swarm_core::SwarmCore::new(vec![
                nexus_singularity::swarm::swarm_node::SwarmNode::new(
                    "Alpha",
                    agent.clone(),
                ),
                nexus_singularity::swarm::swarm_node::SwarmNode::new(
                    "Beta",
                    agent.clone(),
                ),
                nexus_singularity::swarm::swarm_node::SwarmNode::new(
                    "Gamma",
                    agent.clone(),
                ),
            ]),
        )),
    ));

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
        scheduler_tx,
        arbiter: Arc::new(nexus_singularity::arbiter_agent::ArbiterAgent::new(
            agent.clone(),
        )),
        workspace_root: std::env::current_dir().unwrap_or_default(),
        vmo_state,
    });

    // ── BACKGROUND: HTTP SERVER ──────────────────────────────────────────────
    let s_state = state.clone();
    tokio::spawn(async move {
        start_server(s_state).await;
    });

    // ── BACKGROUND: DREAM CYCLE ──────────────────────────────────────────────
    // Synthesises memory strands into system insights every 15 minutes.
    let dream_memory = state.memory.clone();
    let dream_agent = state.agent.clone();
    let dream_key = api_key.clone();
    tokio::spawn(async move {
        nexus_singularity::dream_cycle::start_daydream(
            dream_memory,
            dream_agent,
            dream_key,
        )
        .await;
    });

    // ── BACKGROUND: KNOWLEDGE BROKER ────────────────────────────────────────
    // Indexes the project's public symbols on startup so context-injection in
    // the CLI and API handler has a fresh atlas.
    tokio::spawn(async move {
        nexus_singularity::broker::KnowledgeBroker::refresh_atlas().await;
        println!(
            "\x1b[34m>> [BROKER] Project atlas indexed.\x1b[0m"
        );
    });

    // ── FOREGROUND: CLI LOOP ─────────────────────────────────────────────────
    if let Err(e) = run_cli_loop(state).await {
        eprintln!("!! [FATAL] CLI crashed: {}", e);
    }
}
