use nexus_singularity::mission_control::upgrade_command;
use nexus_singularity::server::AppState;
use nexus_singularity::neural_agent::MicroAgent;
use nexus_singularity::orchestrator::Orchestrator;
use nexus_singularity::scheduler;
use nexus_singularity::system_monitor::SystemMonitor;
use nexus_singularity::arbiter_agent::ArbiterAgent;
use nexus_singularity::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::broadcast;
use std::sync::atomic::AtomicU32;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    println!("\x1b[36m>> [UPGRADE_BINARY] Orchestrating NEXUS v1.7.1-lean Transition...\x1b[0m");

    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let (tx, _) = broadcast::channel::<String>(32);
    
    let agent = Arc::new(MicroAgent::init(tx.clone()).unwrap());
    let monitor = SystemMonitor::new();
    let orchestrator = Arc::new(Orchestrator::new(agent.clone()));
    let scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(), reqwest::Client::new(), api_key.clone(), 
        orchestrator.clone(), Arc::new(AtomicU32::new(0)), monitor.clone()
    );

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor,
        agent,
        orchestrator,
        local_llm: Arc::new(nexus_singularity::local_llm::LocalLLM::new()),
        arbiter: Arc::new(ArbiterAgent::new(Arc::new(MicroAgent::init(tx.clone()).unwrap()), None)),
        memory: Arc::new(tokio::sync::Mutex::new(MemoryPalace::init(std::path::PathBuf::from(".nexus_memory")))),
        api_key,
        current_resonance: Arc::new(AtomicU32::new(1.0f32.to_bits())),
        tx: tx.clone(),
        scheduler_tx,
        workspace_root: std::env::current_dir()?,
    });

    let result = upgrade_command(state).await;
    println!("{}", result);

    Ok(())
}
