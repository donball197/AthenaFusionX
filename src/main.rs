use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::path::PathBuf;

// Inject the universal UI bridge into the binary
mod api_bridge; 

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
    
    // FIX: Await the async init and pass the correct string literal type
    let memory_palace = memory::MemoryPalace::init(".nexus_memory").await?;
    let memory = Arc::new(Mutex::new(memory_palace));
    
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

    // 5. Ignite the Svelte UI API Bridge in the background
    tokio::spawn(async {
        api_bridge::start_api_bridge().await;
    });

    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK READY. IGNITING KERNEL SERVER...\x1b[0m");
    
    // 6. Start the Core Server
    server::start_server(state).await;

    Ok(())
}
