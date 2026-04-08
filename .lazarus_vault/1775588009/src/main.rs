use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, Mutex};
use std::path::PathBuf;
use nexus_singularity::{
    server, arbiter_agent, local_llm, neural_agent, system_monitor,
    orchestrator, memory, scheduler, task_manager, cli,
};
use std::io::{self, Write};

fn get_thermal_string() -> String {
    let temp = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|t| t / 1000.0)
        .unwrap_or(0.0);

    let color = if temp > 75.0 { "\x1b[31m" } else if temp > 60.0 { "\x1b[33m" } else { "\x1b[32m" };
    format!("{}[{:.1}°C]\x1b[0m", color, temp)
}

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

    let server_state = state.clone();
    tokio::spawn(async move {
        server::start_server(server_state).await;
    });

    let muscle = cli::detect_host_muscle();
    println!("\n\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;37mNEXUS COMMAND INTERFACE ONLINE\x1b[0m \x1b[38;5;240m(v2.1.2-Zenith)\x1b[0m                             \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[38;5;118mHOST SEATED:\x1b[0m {:<57} \x1b[1;36m║\x1b[0m", cli::get_muscle_string(&muscle));
    println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");
    
    loop {
        let thermal = get_thermal_string();
        print!("\x1b[38;5;51m│\x1b[0m {} \x1b[38;5;51m❯\x1b[0m ", thermal);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() { break; }
        let trimmed = input.trim();

        if trimmed == "exit" || trimmed == "quit" { break; }
        if trimmed == "purge" {
            let walker = nexus_singularity::void_walker::VoidWalker::new();
            let handle = nexus_singularity::component_runner::AgentComponent::spawn("VOID_WALKER", nexus_singularity::component_runner::Capabilities::ALL, agent.clone(), tx.clone());
            println!("\x1b[35m>> [VOID_WALKER] Awakening...\x1b[0m");
            match walker.hunt_dead_code(&handle).await {
                Ok(report) => println!("{}", report),
                Err(e) => println!("\x1b[31m!! [VOID_WALKER_FAULT]: {}\x1b[0m", e),
            }
            continue;
        }
        if trimmed.is_empty() { continue; }

        if trimmed.starts_with("task ") {
            let directive = trimmed.strip_prefix("task ").unwrap().trim().to_string();
            task_manager::send_task(
                agent.clone(), &reqwest::Client::new(), &api_key, &directive, memory.clone()
            ).await;
        }
    }
    Ok(())
}
