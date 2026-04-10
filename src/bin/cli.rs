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


use anyhow::Result;
use nexus_singularity::{cli, memory, neural_agent, task_manager};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();

    // SUTURE: Binary needs its own broadcast channel for streaming
    let (tx, _) = broadcast::channel(16);

    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let memory_palace = memory::MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    // Simple CLI logic for the standalone bin
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: nexus <directive>");
        return Ok(());
    }

    let raw_directive = args[1..].join(" ");
    let enriched_directive = cli::inject_workspace_dna(&raw_directive);

    println!("\x1b[36m>> [CLI_UPLINK] Sending directive to NEXUS Cortex...\x1b[0m");

    // SUTURE: Added the 6th argument (tx)
    let _ =
        task_manager::send_task(agent, &client, &api_key, &enriched_directive, memory, tx).await;

    Ok(())
}
