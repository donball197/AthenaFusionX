// src/main.rs - v3.1 [Robust Input Suture]
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, Mutex};
use std::io::Write;
use nexus_singularity::{
    server, arbiter_agent, local_llm, neural_agent, system_monitor,
    orchestrator, memory, scheduler, task_manager, cli, metrics,
};

fn get_thermal_string(monitor: &system_monitor::SystemMonitor) -> String {
    let temp = monitor.get_vitals().temperature;
    let color = if temp > 75.0 { "\x1b[31m" } else if temp > 60.0 { "\x1b[33m" } else { "\x1b[32m" };
    if temp == 0.0 {
        format!("\x1b[38;5;240m[NO_TEMP]\x1b[0m")
    } else {
        format!("{}[{:.1}°C]\x1b[0m", color, temp)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let _auth_key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    #[cfg(not(debug_assertions))]
    if _auth_key.is_empty() || auth_key == "SOVEREIGN" {
        eprintln!("\x1b[31m!! [BOOT_GUARD] FATAL: NEXUS_AUTH_KEY is insecure.\x1b[0m");
        std::process::exit(1);
    }

    metrics::init_metrics();
    let (tx, _) = broadcast::channel(512);
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let active_model = std::env::var("NEXUS_MODEL").unwrap_or_else(|_| "gemma-4-26b-a4b-it".to_string());
    
    println!("\x1b[38;5;51m>> [NEXUS] Active model: {}\x1b[0m", active_model);

    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let monitor = system_monitor::SystemMonitor::new();
    let orchestrator = Arc::new(orchestrator::Orchestrator::new(agent.clone()));
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));
    let arbiter = Arc::new(arbiter_agent::ArbiterAgent::new(agent.clone(), Some(orchestrator.clone())));

    let scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(), reqwest::Client::new(), api_key.clone(), orchestrator.clone(),
        Arc::new(AtomicU32::new(0)), monitor.clone(), memory.clone(),
    );

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(), monitor: monitor.clone(), agent: agent.clone(),
        orchestrator, memory: memory.clone(), api_key: api_key.clone(),
        current_resonance: Arc::new(AtomicU32::new(0)), tx: tx.clone(),
        scheduler_tx, arbiter, local_llm: Arc::new(local_llm::LocalLLM::new()),
        workspace_root: std::env::current_dir()?,
    });

    let server_state = state.clone();
    tokio::spawn(async move { server::start_server(server_state).await; });

    let muscle = cli::detect_host_muscle();
    println!("\n\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m \x1b[1;37mNEXUS SINGULARITY v3.0.0 — SOVEREIGN MODE ACTIVE\x1b[0m \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m \x1b[38;5;118mHOST:\x1b[0m {:<60} \x1b[1;36m║\x1b[0m", cli::get_muscle_string(&muscle));
    println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");

    loop {
        let thermal = get_thermal_string(&monitor);
        print!("\x1b[38;5;51m│\x1b[0m {} \x1b[38;5;51m❯\x1b[0m ", thermal);
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() { break; }
        let trimmed = input.trim();

        if trimmed.to_lowercase() == "exit" || trimmed.to_lowercase() == "quit" { break; }
        if trimmed.is_empty() { continue; }

        // SUTURE: Case-insensitive and colon-aware task triggering
        let lower = trimmed.to_lowercase();
        if lower.starts_with("task ") || lower.starts_with("task:") {
            let directive = if lower.starts_with("task:") {
                trimmed[5..].trim().to_string()
            } else {
                trimmed[5..].trim().to_string()
            };

            let agent_clone = agent.clone();
            let memory_clone = memory.clone();
            let tx_clone = tx.clone();
            let api_key_clone = api_key.clone();

            task_manager::send_task(
                agent_clone, &reqwest::Client::new(), &api_key_clone,
                &directive, memory_clone, tx_clone
            ).await;
        }
    }
    Ok(())
}
