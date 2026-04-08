use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, Mutex};
use std::path::PathBuf;
use nexus_singularity::{
    server, arbiter_agent, local_llm, neural_agent, system_monitor,
    orchestrator, memory, scheduler, task_manager, cli,
};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    let (tx, _) = broadcast::channel(512);
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let api_key = std::env::var("GEMINI_API_KEY").expect("CRITICAL: GEMINI_API_KEY missing");
    
    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let monitor = system_monitor::SystemMonitor::new();
    let orchestrator = Arc::new(orchestrator::Orchestrator::new(agent.clone()));
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));
    let arbiter = Arc::new(arbiter_agent::ArbiterAgent::new(agent.clone(), Some(orchestrator.clone())));

    let scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(), reqwest::Client::new(), api_key.clone(), orchestrator.clone(),
        Arc::new(AtomicU32::new(0)), monitor.clone(), memory.clone()
    );

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(), monitor, agent: agent.clone(), orchestrator,
        memory: memory.clone(), api_key: api_key.clone(), current_resonance: Arc::new(AtomicU32::new(0)),
        tx: tx.clone(), scheduler_tx, arbiter, local_llm: Arc::new(local_llm::LocalLLM::new()), workspace_root,
    });

    // Spawn server in background
    let server_state = state.clone();
    tokio::spawn(async move {
        server::start_server(server_state).await;
    });

    let muscle = cli::detect_host_muscle();
    let muscle_str = cli::get_muscle_string(&muscle);
    
    println!("\n\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;37mNEXUS COMMAND INTERFACE ONLINE\x1b[0m \x1b[38;5;240m(v2.1.1-Zenith)\x1b[0m                             \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[38;5;118mHOST SEATED:\x1b[0m {:<57} \x1b[1;36m║\x1b[0m", muscle_str);
    println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");
    
    let client = reqwest::Client::new();
    loop {
        print!("\x1b[38;5;51m│\x1b[0m ❯ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() { break; }
        let trimmed = input.trim();

        if trimmed == "exit" || trimmed == "quit" { break; }
        if trimmed.is_empty() { continue; }

        if trimmed.starts_with("task ") {
            let directive = trimmed.strip_prefix("task ").unwrap().trim().to_string();
            // Important: We await here so the prompt doesn't overwrite the output, 
            // but the output is now fast enough that it won't hang.
            task_manager::send_task(
                agent.clone(), &client, &api_key, &directive, memory.clone()
            ).await;
        } else {
            println!(">> Unknown command. Use 'task <directive>' or 'exit'.");
        }
    }
    Ok(())
}
