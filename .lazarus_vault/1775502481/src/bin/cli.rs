use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, Mutex};
use std::path::PathBuf;
use nexus_singularity::{
    arbiter_agent, local_llm, neural_agent, system_monitor,
    orchestrator, memory, scheduler, self_healing_engine, task_manager, cli,
};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    let (tx, _) = broadcast::channel(512);
    let api_key = std::env::var("GEMINI_API_KEY").expect("CRITICAL: GEMINI_API_KEY missing");
    
    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let monitor = system_monitor::SystemMonitor::new();
    let orchestrator = Arc::new(orchestrator::Orchestrator::new(agent.clone()));
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));
    
    // Check Hardware Muscle bounds
    let muscle = cli::detect_host_muscle();
    let muscle_str = cli::get_muscle_string(&muscle);
    
    println!("\n\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;37mNEXUS COMMAND INTERFACE ONLINE\x1b[0m \x1b[38;5;240m(v2.0.0-Sovereign)\x1b[0m                           \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[38;5;118mHOST SEATED:\x1b[0m {:<57} \x1b[1;36m║\x1b[0m", muscle_str);
    println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");
    
    let client = reqwest::Client::new();
    
    loop {
        // Render HUD Bubble
        println!("\x1b[38;5;51m╭───\x1b[0m \x1b[1;38;5;255mNEXUS_CLI\x1b[0m \x1b[38;5;51m────────────────────────────────────────\x1b[0m");
        print!("\x1b[38;5;51m│\x1b[0m ❯ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).unwrap();
        
        // Handle EOF (Ctrl+D) gracefully
        if bytes_read == 0 { break; }

        let trimmed = input.trim();
        println!("\x1b[38;5;51m╰────────────────────────────────────────────────────────\x1b[0m\n");

        if trimmed == "exit" || trimmed == "quit" { break; }

        if trimmed.starts_with("task ") {
            let directive = trimmed.strip_prefix("task ").unwrap().trim();
            let enriched_directive = cli::inject_workspace_dna(directive);

            let _ = task_manager::send_task(
                agent.clone(), &client, &api_key, &enriched_directive, memory.clone()
            ).await;
        }
    }
    Ok(())
}
