/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/main.rs - v4.1.0 (Ghost Threading & Asynchronous CLI)
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

    // === Auth boot guard ===
    let auth_key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    #[cfg(not(debug_assertions))]
    if auth_key.is_empty() || auth_key == "SOVEREIGN" {
        eprintln!("\x1b[31m");
        eprintln!("!! [BOOT_GUARD] FATAL: NEXUS_AUTH_KEY is default 'SOVEREIGN'.");
        eprintln!("!! Set a strong key:  NEXUS_AUTH_KEY=$(openssl rand -hex 32)");
        eprintln!("\x1b[0m");
        std::process::exit(1);
    }
    #[cfg(debug_assertions)]
    if auth_key.is_empty() || auth_key == "SOVEREIGN" {
        eprintln!("\x1b[33m>> [BOOT_WARN] NEXUS_AUTH_KEY is default — dev only.\x1b[0m");
    }

    metrics::init_metrics();

    let (tx, _) = broadcast::channel(512);
    let api_key = std::env::var("GEMINI_API_KEY")
        .expect("CRITICAL: GEMINI_API_KEY environment variable is missing");

    let active_model = std::env::var("NEXUS_MODEL")
        .unwrap_or_else(|_| "gemma-4-26b-a4b-it (default)".to_string());
    println!("\x1b[38;5;51m>> [NEXUS] Active model: {}\x1b[0m", active_model);

    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let monitor = system_monitor::SystemMonitor::new();
    let orchestrator = Arc::new(orchestrator::Orchestrator::new(agent.clone()));
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    let arbiter = Arc::new(arbiter_agent::ArbiterAgent::new(
        agent.clone(), Some(orchestrator.clone())
    ));

    let scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(),
        reqwest::Client::new(),
        api_key.clone(),
        orchestrator.clone(),
        Arc::new(AtomicU32::new(0)),
        monitor.clone(),
        memory.clone(),
    );

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        monitor: monitor.clone(),
        agent: agent.clone(),
        orchestrator,
        memory: memory.clone(),
        api_key: api_key.clone(),
        current_resonance: Arc::new(AtomicU32::new(0)),
        tx: tx.clone(),
        scheduler_tx,
        arbiter,
        local_llm: Arc::new(local_llm::LocalLLM::new()),
        workspace_root: std::env::current_dir()?,
    });

    let server_state = state.clone();
    tokio::spawn(async move { server::start_server(server_state).await; });

    let muscle = cli::detect_host_muscle();
    println!("\n\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m \x1b[1;37mNEXUS SINGULARITY v4.0.0 — SOVEREIGN MODE ACTIVE\x1b[0m \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m \x1b[38;5;118mHOST:\x1b[0m {:<60} \x1b[1;36m║\x1b[0m", cli::get_muscle_string(&muscle));
    println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");

    loop {
        let thermal = get_thermal_string(&monitor);
        print!("\x1b[38;5;51m│\x1b[0m {} \x1b[38;5;51m❯\x1b[0m ", thermal);
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() { break; }
        let trimmed = input.trim();

        if trimmed == "exit" || trimmed == "quit" { break; }
        if trimmed.is_empty() { continue; }

        if trimmed == "metrics" {
            print!("{}", metrics::get_metrics_text());
            continue;
        }

        if let Some(directive_str) = trimmed.strip_prefix("task ") {
            let directive = directive_str.trim().to_string();
            
            // SUTURE: Clone state for the background thread boundary
            let agent_clone = agent.clone();
            let memory_clone = memory.clone();
            let tx_clone = tx.clone();
            let api_key_clone = api_key.clone();
            
            // SUTURE: Ghost Thread Initialization
            tokio::spawn(async move {
                let _ = task_manager::send_task(
                    agent_clone,
                    &reqwest::Client::new(),
                    &api_key_clone,
                    &directive,
                    memory_clone,
                    tx_clone
                ).await;
                println!("\n\x1b[32m>> [GHOST] Task session finalized. Press Enter to refresh prompt.\x1b[0m");
            });
        }
    }

    Ok(())
}
