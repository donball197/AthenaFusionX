#!/bin/bash
# Project Nexus Singularity - Zenith Alignment Script
echo ">> [NEXUS] INITIATING SYSTEM-WIDE TYPE ALIGNMENT"

# --- 1. REPAIR neural_agent.rs (Fixes Sized [u8] errors) ---
echo "[*] Aligning src/neural_agent.rs..."
cat <<EOF > src/neural_agent.rs
use crate::nexus_error::NexusError;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use reqwest::Client;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::broadcast;
use futures_util::StreamExt;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContextPacket {
    pub current_file: String,
    pub recent_logs: Vec<String>,
    pub file_content: Option<String>,
    pub terminal_log: Option<String>,
    pub system_prompt: Option<String>,
    pub target_model: Option<String>,
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn init(tx: broadcast::Sender<String>) -> Result<Self, NexusError> {
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(16)), tx })
    }

    pub async fn prepare_and_run(
        &self,
        client: &Client,
        api_key: &str,
        user_text: &str,
        ctx: ContextPacket,
        _resonance: f32,
    ) -> Result<(String, u128), NexusError> {
        let start = std::time::Instant::now();
        let model = ctx.target_model.unwrap_or_else(|| "gemma-3-27b-it".to_string());
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, api_key);
        let payload = json!({ "contents": [{ "parts": [{ "text": user_text }] }] });
        
        let response = client.post(url).json(&payload).send().await
            .map_err(|e| NexusError::Other(e.to_string()))?;

        let mut full_text = String::new();
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            if let Ok(bytes_chunk) = item {
                // bytes_chunk is a Bytes object, which is Sized.
                let chunk_str = String::from_utf8_lossy(&bytes_chunk);
                for line in chunk_str.lines() {
                    if line.starts_with("data: ") {
                        if let Ok(val) = serde_json::from_str::<Value>(&line[6..]) {
                            if let Some(token) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                                full_text.push_str(token);
                                let _ = self.tx.send(json!({"type": "ghost_token", "payload": token}).to_string());
                            }
                        }
                    }
                }
            }
        }

        let duration = start.elapsed().as_millis();
        Ok((full_text, duration))
    }
}
EOF

# --- 2. REPAIR orchestrator.rs (Fixes str vs String mismatch) ---
echo "[*] Aligning src/orchestrator.rs..."
cat <<EOF > src/orchestrator.rs
use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::executor::Executor;
use crate::agent_protocol::AgentAction;
use anyhow::Result;
use std::sync::Arc;
use crate::system_monitor::SystemMonitor;

pub struct Orchestrator {
    pub agent: Arc<MicroAgent>,
}

impl Orchestrator {
    pub fn new(agent: Arc<MicroAgent>) -> Self { Self { agent } }

    pub async fn run_autonomous_loop(
        &self, 
        instruction: String, 
        context: ContextPacket,
        client: &reqwest::Client,
        api_key: &str,
        _tx: &tokio::sync::broadcast::Sender<String>,
        _monitor: &SystemMonitor
    ) -> Result<String> {
        let (raw_response, _) = self.agent.prepare_and_run(
            client, 
            api_key, 
            &instruction, 
            context.clone(), 
            1.0
        ).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;

        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&raw_response) {
            let results: Vec<String> = Executor::execute(actions).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
            return Ok(results.join("\n"));
        }

        Ok(raw_response)
    }
}
EOF

# --- 3. REPAIR task_manager.rs (Fixes Type Inference E0282) ---
echo "[*] Aligning src/task_manager.rs..."
cat <<EOF > src/task_manager.rs
use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::json_parser::NeuralParser;
use crate::executor::Executor;
use crate::agent_protocol::AgentAction;
use crate::component_runner::{AgentComponent, Capabilities};
use std::process::Command;
use std::collections::HashSet;
use std::sync::Arc;

pub async fn send_task(agent: Arc<MicroAgent>, client: &reqwest::Client, api_key: &str, directive: &str) {
    let spatial_map = Command::new("find")
        .args(["src", "-maxdepth", "2", "-name", "*.rs"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut observation_log = String::new();
    let mut history: HashSet<String> = HashSet::new();

    let capabilities = Capabilities::READ_FS | Capabilities::WRITE_FS;
    let component = AgentComponent::spawn("Task_Loop_Node", capabilities, agent.clone(), agent.tx.clone());

    for turn in 1..=6 {
        let full_prompt = format!("MISSION: {}\nOBSERVATIONS: {}\nDNA_MAP: {}", directive, observation_log, spatial_map);
        let ctx = ContextPacket::default();

        match component.agent.prepare_and_run(client, api_key, &full_prompt, ctx, 0.1).await {
            Ok((response, _)) => {
                if history.contains(&response) { break; }
                history.insert(response.clone());

                if let Ok(actions) = NeuralParser::extract_actions(&response) {
                    let mut safe_actions: Vec<AgentAction> = Vec::new();
                    for action in actions {
                        if component.validate_action(&action).is_ok() {
                            safe_actions.push(action);
                        }
                    }

                    if let Ok(results) = Executor::execute(safe_actions).await {
                        for res in results {
                            observation_log.push_str(&format!("\n[Turn {}]: {}", turn, res));
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}
EOF

# --- 4. REPAIR main.rs (Fixes Argument Mismatch E0061/E0599) ---
echo "[*] Aligning src/main.rs..."
cat <<EOF > src/main.rs
pub mod server;
pub mod arbiter_agent;
pub mod local_llm;
pub mod neural_agent;
pub mod scheduler;
pub mod system_monitor;
pub mod orchestrator;
pub mod memory;
pub mod task_manager;
pub mod nexus_error;
pub mod agent_protocol;
pub mod executor;
pub mod json_parser;
pub mod component_runner;
pub mod diff;
pub mod thermal_governor;

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let (tx, _) = broadcast::channel(100);
    let (scheduler_tx, _) = mpsc::channel(100);

    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
    
    // Core construction with required arguments
    let agent = Arc::new(neural_agent::MicroAgent::init(tx.clone())?);
    let monitor = system_monitor::SystemMonitor::new();
    let orchestrator = Arc::new(orchestrator::Orchestrator::new(agent.clone()));
    let memory = Arc::new(Mutex::new(memory::MemoryPalace::init(PathBuf::from(".nexus_memory"))));
    let arbiter = Arc::new(arbiter_agent::ArbiterAgent::new(agent.clone(), Some(orchestrator.clone())));

    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        monitor,
        agent,
        orchestrator,
        memory,
        api_key,
        current_resonance: Arc::new(AtomicU32::new(0)),
        tx,
        scheduler_tx,
        arbiter,
        local_llm: Arc::new(local_llm::LocalLLM::new()),
        workspace_root,
    });

    server::start_server(state).await;
    Ok(())
}
EOF

echo ">> [NEXUS] ARCHITECTURE REALIGNMENT COMPLETE."
echo ">> Run 'cargo check' to verify stability."

