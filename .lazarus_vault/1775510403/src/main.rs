use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, Mutex};
use std::path::PathBuf;
use nexus_singularity::{
    server, arbiter_agent, local_llm, neural_agent, system_monitor,
    orchestrator, memory, scheduler, self_healing_engine, task_manager, cli,
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

    let engine_state = state.clone();
    let engine_memory = memory.clone();
    tokio::spawn(async move {
        let healing_engine = self_healing_engine::SelfHealingEngine::new(engine_state, engine_memory);
        healing_engine.ignite_autonomic_loop().await;
    });

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "serve" {
        println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK READY. GEMMA-4 CORE + SELF-HEALING ENGAGED.\x1b[0m");
        server::start_server(state).await;
    } else {
        // === SUTURE: HARDWARE DETECTION & HUD BANNER ===
        let muscle = cli::detect_host_muscle();
        let muscle_str = cli::get_muscle_string(&muscle);
        
        println!("\n\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
        println!("\x1b[1;36m║\x1b[0m  \x1b[1;37mNEXUS COMMAND INTERFACE ONLINE\x1b[0m \x1b[38;5;240m(v2.0.0-Sovereign)\x1b[0m                           \x1b[1;36m║\x1b[0m");
        println!("\x1b[1;36m║\x1b[0m  \x1b[38;5;118mHOST SEATED:\x1b[0m {:<57} \x1b[1;36m║\x1b[0m", muscle_str);
        println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");
        
        let client = reqwest::Client::new();
        loop {
            // === SUTURE: THE BUBBLE PROMPT ===
            println!("\x1b[38;5;51m╭───\x1b[0m \x1b[1;38;5;255mNEXUS_CLI\x1b[0m \x1b[38;5;51m────────────────────────────────────────\x1b[0m");
            print!("\x1b[38;5;51m│\x1b[0m ❯ ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim();

            println!("\x1b[38;5;51m╰────────────────────────────────────────────────────────\x1b[0m\n");

            if input == "exit" || input == "quit" { break; }

            if input.starts_with("task ") {
                let directive = input.strip_prefix("task ").unwrap().trim();
                let enriched_directive = cli::inject_workspace_dna(directive);

                let _ = task_manager::send_task(
                    agent.clone(), &client, &api_key, &enriched_directive, memory.clone()
                ).await;
            }
        }
    }
    Ok(())
}
