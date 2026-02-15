#!/bin/bash

echo ">> [NEXUS] APPLYING INTEGRATION PATCH..."

# 1. UPDATE SYSTEM MONITOR (Ensure it is public and accessible)
cat > src/system_monitor.rs << 'EOF'
use std::fs;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub ram_usage: f32,
    pub ram_total: u64,
    pub ram_used: u64,
}

#[derive(Clone)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor
    }

    pub fn get_stats(&self) -> SystemStats {
        let (cpu, _) = self.calculate_cpu();
        let (used, total, percent) = self.get_memory();
        
        SystemStats {
            cpu_usage: cpu,
            ram_usage: percent,
            ram_total: total,
            ram_used: used,
        }
    }

    fn calculate_cpu(&self) -> (f32, u64) {
        // Simple /proc/stat parser
        if let Ok(content) = fs::read_to_string("/proc/stat") {
            if let Some(line) = content.lines().next() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 8 {
                    let user: f32 = parts[1].parse().unwrap_or(0.0);
                    let idle: f32 = parts[4].parse().unwrap_or(0.0);
                    // Extremely basic load calculation for demo stability
                    // A real implementation requires diffing against previous tick
                    return ((user / (user + idle)) * 100.0, 0); 
                }
            }
        }
        (0.0, 0)
    }

    fn get_memory(&self) -> (u64, u64, f32) {
        let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mut total = 0;
        let mut available = 0;
        
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) * 1024;
            } else if line.starts_with("MemAvailable:") {
                available = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) * 1024;
            }
        }
        
        if total == 0 { return (0, 0, 0.0); }
        let used = total.saturating_sub(available);
        let percent = (used as f32 / total as f32) * 100.0;
        
        (used, total, percent)
    }
}
EOF

# 2. UPDATE SERVER (Remove unused imports & Connect Monitor)
cat > src/server.rs << 'EOF'
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State, Query},
    routing::{get, post},
    Json, Router, http::{StatusCode, Method},
};
use std::sync::Arc;
use std::fs;
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use tower_http::cors::{CorsLayer, Any};
use tower_http::services::ServeDir;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use futures_util::{sink::SinkExt, stream::StreamExt};
// Unused imports removed: AsyncReadExt, AsyncWriteExt

use crate::system_monitor::SystemMonitor;

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor, // Added Monitor to State
}

#[derive(Deserialize)]
pub struct FileParams { pub path: String }

#[derive(Deserialize, Serialize)]
pub struct FileContent { pub path: String, pub content: String }

#[derive(Deserialize, Serialize)]
pub struct PromptRequest { pub prompt: String }

#[derive(Deserialize, Serialize)]
pub struct NeuralResponse { pub response: String, pub status: String }

#[derive(Serialize)]
pub struct Vitals {
    pub status: String,
    pub cpu_usage: f32,
    pub ram_usage: String,
    pub ram_percent: f32,
    pub integrity: u32,
}

pub async fn start_server(state: Arc<AppState>) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/vitals", get(handle_vitals))
        .route("/api/prompt", post(handle_prompt))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("ui/dist"))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!(">> [NEXUS] Sovereign Bridge Online: http://localhost:3000");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// Updated to use Real System Monitor
async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<Vitals> {
    let stats = state.monitor.get_stats();
    
    // Format RAM string (e.g., "1.2GB / 4.0GB")
    let used_gb = stats.ram_used as f32 / 1_073_741_824.0;
    let total_gb = stats.ram_total as f32 / 1_073_741_824.0;

    Json(Vitals {
        status: "ONLINE".to_string(),
        cpu_usage: stats.cpu_usage,
        ram_usage: format!("{:.1}GB / {:.1}GB", used_gb, total_gb),
        ram_percent: stats.ram_usage,
        integrity: 100,
    })
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let pty_system = native_pty_system();
    // 80x24 is standard terminal size
    let pair = pty_system.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
        .expect("Failed to open PTY");

    let cmd = CommandBuilder::new("/bin/bash");
    let mut child = pair.slave.spawn_command(cmd).expect("Failed to spawn shell");
    
    let mut reader = pair.master.try_clone_reader().expect("Failed to get reader");
    let mut writer = pair.master.take_writer().expect("Failed to get writer");

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Spawn async tasks for bidirectional communication
    let mut pty_to_ws = tokio::task::spawn_blocking(move || {
        let mut buffer = [0u8; 1024];
        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 { break; }
            let msg = String::from_utf8_lossy(&buffer[..n]).to_string();
            // Blocking send not ideal but functional for PTY
            // In full async implementation we would use a channel
        }
    });

    // NOTE: Full PTY implementation requires more complex async bridging
    // For now, keeping the build safe. Use 'cargo run' to see logic.
}

async fn handle_read_file(Query(params): Query<FileParams>) -> Result<Json<FileContent>, StatusCode> {
    if params.path.contains("..") || params.path.starts_with("/") { return Err(StatusCode::FORBIDDEN); }
    let target = if params.path.is_empty() { "src/main.rs" } else { &params.path };
    match fs::read_to_string(target) {
        Ok(content) => Ok(Json(FileContent { path: target.to_string(), content })),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> StatusCode {
    if payload.path.contains("..") || payload.path.starts_with("/") { return StatusCode::FORBIDDEN; }
    if fs::write(&payload.path, &payload.content).is_ok() {
        println!(">> [IDE] Saved changes to {}", payload.path);
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn handle_prompt(State(_state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    // In future: connect this to state.brain_endpoint
    Json(NeuralResponse {
        response: format!(">> ECHO: {}", payload.prompt),
        status: "ok".to_string(),
    })
}
EOF

# 3. UPDATE MAIN (Inject Monitor)
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
use system_monitor::SystemMonitor; // Import the monitor

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    alloc::initialize_mnemnos_lean();
    
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "NEXUS_LITE".to_string());
    println!(">> [FORGE] NODE [{}] ONLINE. INDEPENDENT MODE.", node_id);

    // 1. Initialize Shared State with Real Monitor
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemini-1.5-flash".to_string(),
        monitor: SystemMonitor::new(), // Inject Monitor Here
    });

    // 2. Spawn HUD Server in background
    let server_state = state.clone();
    tokio::spawn(async move {
        server::start_server(server_state).await;
    });

    // 3. Initialize Mission Logic
    // Allow unused variables to suppress warnings while logic is WIP
    #[allow(unused_variables)]
    let mut hiram = hiram_core::Hiram::new(300);
    #[allow(unused_variables)]
    let mut cortex = brain::NeuralNet::new();
    #[allow(unused_variables)]
    let refactor_engine = refactor::RefactorEngine::new();
    let api_key = env::var("GEMINI_API_KEY").unwrap_or_default();
    let agent = neural_agent::MicroAgent::init()?;

    println!(">> [NEXUS] Core Loop Active. Press Ctrl+C to terminate.");

    loop {
        // Keep the main thread alive
        sleep(Duration::from_secs(30)).await;
    }
}
EOF

echo ">> [NEXUS] PATCH APPLIED. REBUILDING..."
# Rebuild to verify warnings are gone/reduced and binaries link
export CARGO_BUILD_JOBS=1
export RUSTFLAGS="-C codegen-units=1 -C debuginfo=0"
cargo build --release
