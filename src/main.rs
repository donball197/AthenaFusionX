use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::path::PathBuf;

use nexus_singularity::{
    server, 
    arbiter_agent, 
    local_llm, 
    neural_agent, 
    system_monitor, 
    orchestrator, 
    memory, 
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    // 1. Initialize Communication Channels
    let (tx, _) = broadcast::channel(100);
    let (scheduler_tx, _scheduler_rx) = mpsc::channel(100);

    // 2. Prepare Environment
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
    
    // 3. Initialize Core Components
    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let monitor = system_monitor::SystemMonitor::new();
    let orchestrator = Arc::new(orchestrator::Orchestrator::new(agent.clone()));
    let memory = Arc::new(Mutex::new(memory::MemoryPalace::init(PathBuf::from(".nexus_memory"))));
    
    let arbiter = Arc::new(arbiter_agent::ArbiterAgent::new(
        agent.clone(), 
        Some(orchestrator.clone())
    ));

    // 4. Assemble AppState
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        monitor,
        agent,
        orchestrator,
        memory,
        api_key,
        current_resonance: Arc::new(AtomicU32::new(0)),
        tx,
        scheduler_tx,
        arbiter,
        local_llm: Arc::new(local_llm::LocalLLM::new()),
        workspace_root,
    });

    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK READY. IGNITING SERVER...\x1b[0m");
    
    // 5. Start the Server
    server::start_server(state).await;

    Ok(())
}
