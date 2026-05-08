use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use nexus_runtime_core::{
    cluster::ClusterMesh,
    distributed_scheduler::DistributedScheduler,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    task_manager::send_task,
};
use nexus_substrate::memory::MemoryPalace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("nexus=info".parse().unwrap())).init();
    println!("\x1b[36m==================================================\x1b[0m");
    println!("\x1b[1;36m>> NEXUS SINGULARITY v3.0 — SOVEREIGN CLI ONLINE\x1b[0m");
    println!("\x1b[36m==================================================\x1b[0m");

    let api_key = std::env::var("GEMINI_API_KEY").expect("Fatal: GEMINI_API_KEY not found in .env file.");
    let (tx, mut rx) = broadcast::channel::<String>(256);
    let memory       = Arc::new(Mutex::new(MemoryPalace::new("nexus_cli.db").await?));
    let vmo_state    = Arc::new(Mutex::new(VmoState::default()));
    let cluster      = Arc::new(ClusterMesh::new());
    let dist         = Arc::new(DistributedScheduler::new(cluster));
    let orchestrator = Arc::new(Orchestrator::new());
    let client       = reqwest::Client::new();

    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = parsed["type"].as_str().unwrap_or("LOG");
                let payload  = parsed["payload"].as_str().unwrap_or("");
                match msg_type {
                    "NEURAL_THOUGHT" => println!("\n\x1b[35m[CORTEX]\x1b[0m {}", payload),
                    "TASK_STATUS"    => println!("\x1b[33m[SYS]\x1b[0m {}",    payload),
                    "LOG_EVENT"      => println!("\x1b[90m[EVENT]\x1b[0m {}",   payload),
                    _                => {}
                }
            } else {
                println!("\x1b[90m[RAW]\x1b[0m {}", msg);
            }
        }
    });

    println!("\x1b[32m>> Neural Uplink Established. Substrate Ready.\x1b[0m");
    println!("\x1b[90mType '/help' for commands, or enter a directive.\x1b[0m");
    let stdin = io::stdin();

    loop {
        print!("\n\x1b[1;32mnexus>\x1b[0m ");
        io::stdout().flush()?;
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let command = input.trim();
        if command.is_empty() { continue; }

        match command {
            "/exit" | "/quit" => { println!("\x1b[31m>> Severing Uplink. Goodbye.\x1b[0m"); break; }
            "/help" => {
                println!("\x1b[36mCommands:\x1b[0m");
                println!("  /status    — Active VMOs + RAM usage");
                println!("  /clear     — Purge SQLite Memory Palace");
                println!("  /exit      — Shutdown CLI");
                println!("  <text>     — Submit directive to the Neural Agent");
            }
            "/status" => {
                let active = unsafe { nexus_substrate::kernel::ffi::muscle_get_active_vmos() };
                println!("\x1b[36m[SUBSTRATE]\x1b[0m Active VMOs: {}", active);
                if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                    for line in status.lines() {
                        if line.starts_with("VmRSS:") || line.starts_with("VmPeak:") { println!("\x1b[36m[MEM]\x1b[0m {}", line.trim()); }
                    }
                }
            }
            "/clear" => {
                let mem = memory.lock().await;
                if let Err(e) = mem.purge_all().await { println!("\x1b[31m[ERROR]\x1b[0m Memory purge failed: {}", e); } 
                else { println!("\x1b[32m[MEMORY]\x1b[0m Neural context eradicated."); }
            }
            directive => {
                println!("\x1b[36m>> Engaging Neural Routing...\x1b[0m");
                let response = send_task(orchestrator.clone(), &client, &api_key, directive, memory.clone(), vmo_state.clone(), tx.clone(), dist.clone()).await;
                println!("\n\x1b[1;32m[MISSION COMPLETE]\x1b[0m {}", response);
            }
        }
    }
    Ok(())
}
