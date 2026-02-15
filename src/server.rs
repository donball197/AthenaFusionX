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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor,
    pub agent: MicroAgent,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct FileParams { pub path: String }
#[derive(Deserialize, Serialize)]
pub struct FileContent { pub path: String, pub content: String }

// UPDATED: Now includes optional context
#[derive(Deserialize, Serialize)]
pub struct PromptRequest { pub prompt: String, pub context: Option<String> }

#[derive(Deserialize, Serialize)]
pub struct NeuralResponse { pub response: String, pub status: String, pub latency: u128 }

#[derive(Serialize)]
pub struct Vitals {
    pub status: String,
    pub cpu_usage: f32,
    pub ram_usage: String,
    pub ram_percent: f32,
    pub integrity: u32,
}

pub async fn start_server(state: Arc<AppState>) {
    let cors = CorsLayer::new().allow_origin(Any).allow_methods([Method::GET, Method::POST]).allow_headers(Any);
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
    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<Vitals> {
    let stats = state.monitor.get_stats();
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

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    // PASS THE CONTEXT TO THE AGENT
    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, payload.context).await {
        Ok((res, latency)) => {
            Json(NeuralResponse { response: res, status: "ok".to_string(), latency })
        },
        Err(e) => Json(NeuralResponse { response: format!("ERROR: {}", e), status: "fail".to_string(), latency: 0 })
    }
}

// ... PTY Handlers (unchanged) ...
async fn ws_handler(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse { ws.on_upgrade(handle_socket) }
async fn handle_socket(socket: WebSocket) {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }).expect("Failed to open PTY");
    let cmd = CommandBuilder::new("/bin/bash");
    let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn shell");
    let mut reader = pair.master.try_clone_reader().expect("Failed to get reader");
    let mut writer = pair.master.take_writer().expect("Failed to get writer");
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let pty_to_ws = tokio::spawn(async move {
        let mut buffer = [0u8; 1024];
        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 { break; }
            let msg = String::from_utf8_lossy(&buffer[..n]).to_string();
            if ws_sender.send(Message::Text(msg)).await.is_err() { break; }
        }
    });
    let ws_to_pty = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let Message::Text(text) = msg {
                let _ = writer.write_all(text.as_bytes());
                let _ = writer.flush();
            }
        }
    });
    tokio::select! { _ = pty_to_ws => {}, _ = ws_to_pty => {} }
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
    } else { StatusCode::INTERNAL_SERVER_ERROR }
}
