#!/bin/bash

echo ">> [NEXUS] ALIGNING NEURAL PATHWAYS TO GEMMA-3-27B-IT..."

# 1. UPDATE NEURAL AGENT (The Logic Core)
# This forces the API call to target the specific Gemma 3 model.
cat > src/neural_agent.rs << 'EOF'
use anyhow::Result;
use serde_json::json;
use reqwest::Client;

pub struct MicroAgent;

impl MicroAgent {
    pub fn init() -> Result<Self> {
        Ok(MicroAgent)
    }

    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str) -> Result<String> {
        // TARGET: gemma-3-27b-it
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}",
            api_key
        );

        // Payload construction
        let payload = json!({
            "contents": [{
                "parts": [{"text": user_text}]
            }],
            // Optional: Adjust temperature for coding/logic precision
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": 2048
            }
        });

        let resp = client.post(url)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Ok(format!(">> [GEMMA-3 ERROR] API Refused: {}", error_text));
        }

        let body: serde_json::Value = resp.json().await?;
        
        let text = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or(">> [GEMMA-3] Void response.");

        Ok(text.to_string())
    }
    
    // Stub for future evolution features
    pub async fn evolve_kernel(&self, client: &Client, api_key: &str, prompt: &str, _context: Option<&str>) -> Result<String> {
        self.prepare_and_run(client, api_key, prompt).await
    }
}
EOF

# 2. UPDATE MAIN CONFIGURATION
# Updates the AppState so the Server knows which brain is active.
# We also ensure the SystemMonitor is correctly imported here.
cat > src/main.rs << 'EOF'
mod hiram_core;
mod neural_agent;
mod refactor;
mod auditor;
mod broker;
mod janitor;
mod monitor;
mod alloc;
mod sentry;
mod server;
mod memory;
mod brain;
mod system_monitor;
mod vision;
mod calibrate;
mod network;

use tokio::time::{sleep, Duration};
use std::{env, sync::Arc};
use system_monitor::SystemMonitor; 

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    alloc::initialize_mnemnos_lean();
    
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "NEXUS_DUET".to_string());
    // Explicitly announcing the Model Version
    println!(">> [FORGE] NODE [{}] ONLINE. MODEL: GEMMA-3-27B-IT", node_id);

    // 1. Initialize Shared State with correct Brain Endpoint
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(), // ALIGNED
        monitor: SystemMonitor::new(),
    });

    // 2. Spawn HUD Server in background
    let server_state = state.clone();
    tokio::spawn(async move {
        server::start_server(server_state).await;
    });

    // 3. Initialize Mission Logic
    #[allow(unused_variables)]
    let mut hiram = hiram_core::Hiram::new(300);
    #[allow(unused_variables)]
    let mut cortex = brain::NeuralNet::new();
    #[allow(unused_variables)]
    let refactor_engine = refactor::RefactorEngine::new();
    
    // Ensure API Key is loaded
    let _api_key = env::var("GEMMA_API_KEY").unwrap_or_default();
    let _agent = neural_agent::MicroAgent::init()?;

    println!(">> [NEXUS] Core Loop Active. Neural Link: STABLE.");

    loop {
        // Pulse check every 30 seconds
        sleep(Duration::from_secs(30)).await;
    }
}
EOF

# 3. REBUILD TO LOCK IN CHANGES
echo ">> [NEXUS] COMPILING UPDATED CONFIGURATION..."
export CARGO_BUILD_JOBS=1
export RUSTFLAGS="-C codegen-units=1 -C debuginfo=0"
cargo build --release

if [ $? -eq 0 ]; then
    echo ">> [SUCCESS] Nexus is now running on Gemma-3-27B-IT."
    echo ">> [INFO] Run 'cargo run --release' to start."
else
    echo ">> [FAIL] Compilation Error."
fi
