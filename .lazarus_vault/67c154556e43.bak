use nexus_singularity::server::{AppState, start_server};
use nexus_singularity::cli::run_cli_loop;
use nexus_singularity::dream_cycle::start_daydream;
use nexus_singularity::void_walker::VoidWalker;
use nexus_singularity::lazarus_vault::LazarusVault;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::env;

#[tokio::main]
async fn main() {
    println!("\x1b[1;36m>> [NEXUS] IGNITION SEQUENCE INITIATED (v2.0.0)\x1b[0m");
    dotenvy::dotenv().ok();
    let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| "UNSET".to_string());
    let (tx, _) = broadcast::channel(256);
    let (scheduler_tx, _) = mpsc::channel(100);

    let memory = Arc::new(Mutex::new(nexus_singularity::memory::MemoryPalace::new(".nexus_memory.db").await.unwrap()));
    let agent = Arc::new(nexus_singularity::neural_agent::MicroAgent::init(tx.clone()).unwrap());

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor: Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()),
        agent: agent.clone(),
        orchestrator: Arc::new(nexus_singularity::orchestrator::Orchestrator::new(agent.clone(), None)),
        memory: memory.clone(),
        api_key: api_key.clone(),
        current_resonance: Arc::new(std::sync::atomic::AtomicU32::new(50)),
        tx: tx.clone(),
        scheduler_tx,
        arbiter: Arc::new(nexus_singularity::arbiter_agent::ArbiterAgent::new(agent.clone())),
        local_llm: Arc::new(nexus_singularity::local_llm::LocalLLM::new()),
        workspace_root: std::env::current_dir().unwrap_or_default(),
    });

    // 1. [ARCHIVIST] Background Dream Cycle
    let d_mem = memory.clone();
    let d_agent = agent.clone();
    let d_key = api_key.clone();
    tokio::spawn(async move {
        start_daydream(d_mem, d_agent, d_key).await;
    });

    // 2. [SENTINEL] Proactive Defense
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
            let orphans = VoidWalker::identify_orphans();
            for file in orphans {
                println!("\x1b[33m>> [SENTINEL] Unprotected file: {}. Snapshotting.\x1b[0m", file);
                let _ = LazarusVault::auto_snapshot(&file, "sentinel_auto").await;
            }
        }
    });

    // 3. Launch Server
    let s_state = state.clone();
    tokio::spawn(async move { start_server(s_state).await; });

    // 4. Foreground CLI
    if let Err(e) = run_cli_loop(state).await {
        eprintln!("!! [FATAL] CLI crashed: {}", e);
    }
}
