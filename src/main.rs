/**
 * Project Nexus Singularity - v1.9.5 (Ultra-Lean CLI)
 */
use std::sync::Arc;
use tokio::sync::Mutex;
use nexus_singularity::{
    memory::MemoryPalace,
    orchestrator::Orchestrator,
    neural_agent::MicroAgent,
    server::AppState,
    cli,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Hardware Synthesis
    let muscle = cli::detect_host_muscle();
    let muscle_str = cli::get_muscle_string(&muscle);

    print!("\x1B[2J\x1B[1;1H"); 
    println!("\x1b[1;36m>> [NEXUS SINGULARITY] IGNITION SEQUENCE INITIATED\x1b[0m");
    println!("\x1b[38;5;240m>> [HOST_DNA] {}\x1b[0m", muscle_str);

    // 2. Establish Nervous System
    let (tx, _rx) = tokio::sync::broadcast::channel(256);
    let (scheduler_tx, _scheduler_rx) = tokio::sync::mpsc::channel(100);

    // 3. Neural Initialization
    let memory = Arc::new(Mutex::new(MemoryPalace::new(".nexus_brain.db").await?));
    let agent = Arc::new(MicroAgent::init(tx.clone())?);
    let orchestrator = Arc::new(Orchestrator::new(agent.clone(), None));
    
    // 4. State Binding
    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor: tokio::sync::Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()),
        agent: agent.clone(),
        orchestrator,
        memory: memory.clone(),
        api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        current_resonance: Arc::new(std::sync::atomic::AtomicU32::new(528)),
        tx: tx.clone(),
        scheduler_tx,
        arbiter: Arc::new(nexus_singularity::arbiter_agent::ArbiterAgent::new(agent.clone())),
        local_llm: Arc::new(nexus_singularity::local_llm::LocalLLM::new()),
        workspace_root: std::env::current_dir()?,
    });

    // 5. Silent Background Uplink (No Visor pop-ups)
    let server_state = state.clone();
    tokio::spawn(async move {
        nexus_singularity::server::start_server(server_state).await;
    });

    // 6. Immediate CLI Ignition
    println!("\x1b[32m>> [NEXUS] CLI ACTIVE. SYSTEM STANDING BY.\x1b[0m");
    nexus_singularity::cli::run_cli_loop(state).await?;

    Ok(())
}
