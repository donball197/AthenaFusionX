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

use nexus_singularity::arbiter_agent::ArbiterAgent;
use nexus_singularity::memory::MemoryPalace;
use nexus_singularity::mission_control::upgrade_command;
use nexus_singularity::neural_agent::MicroAgent;
use nexus_singularity::orchestrator::Orchestrator;
use nexus_singularity::scheduler;
use nexus_singularity::server::AppState;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    println!("\x1b[36m>> [UPGRADE_BINARY] Orchestrating NEXUS Transition...\x1b[0m");

    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let (tx, _) = broadcast::channel::<String>(128);

    let agent = Arc::new(MicroAgent::init(tx.clone()).unwrap());
    
    // ⚡ Vitals locked in a Mutex for the UI / Server State
    let monitor = tokio::sync::Mutex::new(nexus_singularity::system_monitor::SystemMonitor::new());
    let orchestrator = Arc::new(Orchestrator::new(agent.clone(), None));

    let memory_palace = MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    // ⚡ Cellular Division: The Scheduler gets its own dedicated, raw SystemMonitor
    let scheduler_tx = scheduler::MasterSupervisor::ignite(
        tx.clone(),
        reqwest::Client::new(),
        api_key.clone(),
        orchestrator.clone(),
        Arc::new(AtomicU32::new(0)),
        nexus_singularity::system_monitor::SystemMonitor::new(),
        memory.clone(), // ⚡ Memory correctly seated in the 7th slot
    );

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        monitor, // ⚡ Mutex monitor correctly assigned to AppState
        agent: agent.clone(),
        orchestrator,
        local_llm: Arc::new(nexus_singularity::local_llm::LocalLLM::new()),
        arbiter: Arc::new(ArbiterAgent::new(agent)),
        memory,
        api_key,
        current_resonance: Arc::new(AtomicU32::new(0)),
        tx: tx.clone(),
        scheduler_tx,
        workspace_root: std::env::current_dir()?,
    });

    let result = upgrade_command(state).await;
    println!("{}", result);

    Ok(())
}
