// src/bin/upgrade.rs — v1.9.1 [IMPORT_ALIGNMENT]
use anyhow::Result;
use nexus_substrate::memory::MemoryPalace;
use nexus_singularity::arbiter_agent::ArbiterAgent;
use nexus_singularity::cluster::ClusterMesh;
use nexus_singularity::distributed_scheduler::DistributedScheduler;
use nexus_singularity::mission_control::upgrade_command;
use nexus_singularity::neural_agent::MicroAgent;
use nexus_singularity::orchestrator::Orchestrator;
use nexus_singularity::scheduler;
use nexus_singularity::server::{AppState, VmoState};
use nexus_singularity::governance::protocol::GovernanceLog;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use parking_lot::Mutex as SyncMutex;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    println!("\x1b[36m>> [UPGRADE_BINARY] Orchestrating NEXUS Transition...\x1b[0m");

    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let (tx, _) = broadcast::channel::<String>(128);

    let agent = Arc::new(MicroAgent::init(tx.clone()).unwrap());
    
    let monitor = Arc::new(tokio::sync::Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new()));

    let orchestrator = Arc::new(Orchestrator::new());

    let memory_palace = MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));

    let cluster = Arc::new(ClusterMesh::new());
    let dist = Arc::new(DistributedScheduler::new(cluster.clone()));
    
    let governance_log = Arc::new(SyncMutex::new(GovernanceLog::init()));

    let scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(),
        reqwest::Client::new(),
        api_key.clone(),
        orchestrator.clone(),
        Arc::new(AtomicU32::new(0)),
        nexus_singularity::system_monitor::SystemMonitor::new(),
        memory.clone(),
        vmo_state.clone(),
        dist.clone(),
    );

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor: monitor.clone(),
        agent: agent.clone(),
        orchestrator,
        arbiter: Arc::new(ArbiterAgent::new(agent)),
        memory,
        api_key,
        current_resonance: Arc::new(AtomicU32::new(0)),
        tx: tx.clone(),
        scheduler_tx,
        workspace_root: std::env::current_dir()?,
        vmo_state,
        cluster,
        dist,
        governance_log,
    });

    let result = upgrade_command(state).await;
    println!("{}", result);

    Ok(())
}
