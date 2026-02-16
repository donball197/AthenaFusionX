use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State, Query},
    routing::{get, post, delete},
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

use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::{MemoryPalace, Memory};

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor,
    pub agent: MicroAgent,
    pub memory: MemoryPalace,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct FileParams { pub path: String }
#[derive(Deserialize, Serialize)]
pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)]
pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String }

#[derive(Deserialize, Serialize)]
pub struct PromptRequest { 
    pub prompt: String, 
    pub context: Option<String>,
    pub terminal_context: Option<String> 
}

#[derive(Deserialize)]
pub struct MemoryRequest { pub prompt: String, pub code: String }
#[derive(Deserialize)]
pub struct PruneRequest { pub keep_keyword: String }
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
    let cors = CorsLayer::new().allow_origin(Any).allow_methods([Method::GET, Method::POST, Method::DELETE]).allow_headers(Any);

    let app = Router::new()
        .route("/api/vitals", get(handle_vitals))
        .route("/api/prompt", post(handle_prompt))
        .route("/api/memory", post(handle_memory))
        .route("/api/memories", get(handle_get_memories))
        .route("/api/memories/purge", delete(handle_purge_memories))
        .route("/api/memories/archive", post(handle_archive_memories))
        .route("/api/memories/prune", post(handle_prune_memories))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file))
        .route("/api/fs/list", get(handle_list_files))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("ui/dist"))
        .with_state(state).layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!(">> [NEXUS] Sovereign Node Online: http://localhost:3000");
    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<Vitals> {
    let stats = state.monitor.get_stats();
    let used_gb = stats.ram_used as f32 / 1_073_741_824.0;
    let total_gb = stats.ram_total as f32 / 1_073_741_824.0;
    Json(Vitals { status: "ONLINE".to_string(), cpu_usage: stats.cpu_usage, ram_usage: format!("{:.1}GB / {:.1}GB", used_gb, total_gb), ram_percent: stats.ram_usage, integrity: 100 })
}

async fn handle_memory(State(state): State<Arc<AppState>>, Json(payload): Json<MemoryRequest>) -> StatusCode {
    state.memory.remember(payload.prompt, payload.code); StatusCode::OK
}

async fn handle_get_memories(State(state): State<Arc<AppState>>) -> Json<Vec<Memory>> { Json(state.memory.load_all()) }

async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> StatusCode { 
    state.memory.purge(); 
    println!(">> [CORTEX] Memories purged.");
    StatusCode::OK 
}

async fn handle_archive_memories(State(state): State<Arc<AppState>>) -> StatusCode {
    let memories = state.memory.load_all();
    let timestamp = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let filename = format!("nexus_archive_{}.json", timestamp);
    if let Ok(json) = serde_json::to_string_pretty(&memories) {
        if fs::write(&filename, json).is_ok() { 
            println!(">> [SYSTEM] ARCHIVE CREATED: {}", filename);
            return StatusCode::OK; 
        }
    }
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn handle_prune_memories(State(state): State<Arc<AppState>>, Json(payload): Json<PruneRequest>) -> StatusCode {
    let mut memories = state.memory.load_all();
    memories.retain(|m| m.prompt.to_lowercase().contains(&payload.keep_keyword.to_lowercase()) || m.solution_code.to_lowercase().contains(&payload.keep_keyword.to_lowercase()));
    if let Ok(json) = serde_json::to_string_pretty(&memories) {
        let _ = fs::write("nexus_memories.json", json); return StatusCode::OK;
    }
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    let mut final_context = payload.context.clone().unwrap_or_default();
    let is_swarm = payload.prompt.to_lowercase().contains("swarm");
    let is_audit = payload.prompt.to_lowercase().contains("audit");
    let mut actual_prompt = payload.prompt.clone();
    if is_swarm { actual_prompt = format!("SWARM_ANALYSIS: Coordinate multi-vector review: {}", payload.prompt); }
    else if is_audit { actual_prompt = "SELF_AUDIT: Summarize progress and suggest focus areas.".to_string(); }
    let memory_context = state.memory.recall();
    if !memory_context.is_empty() { final_context.push_str("\n\n=== RECALLED ===\n"); final_context.push_str(&memory_context); }
    if let Some(logs) = payload.terminal_context { final_context.push_str("\n\n=== TERMINAL ===\n"); final_context.push_str(&logs); }
    match state.agent.prepare_and_run(&state.client, &state.api_key, &actual_prompt, Some(final_context)).await {
        Ok((res, latency)) => {
            let prefix = if is_swarm { ">> [SWARM]\n" } else if is_audit { ">> [AUDIT]\n" } else { "" };
            Json(NeuralResponse { response: format!("{}{}", prefix, res), status: "ok".to_string(), latency })
        },
        Err(e) => Json(NeuralResponse { response: format!("ERROR: {}", e), status: "fail".to_string(), latency: 0 })
    }
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse { ws.on_upgrade(handle_socket) }

async fn handle_socket(socket: WebSocket) {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }).expect("PTY error");
    let cmd = CommandBuilder::new("/bin/bash");
    let _child = pair.slave.spawn_command(cmd).unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let pty_to_ws = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        while let Ok(n) = std::io::Read::read(&mut reader, &mut buf) {
            if n == 0 { break; }
            let msg = String::from_utf8_lossy(&buf[..n]).to_string();
            if ws_sender.send(Message::Text(msg)).await.is_err() { break; }
        }
    });
    let ws_to_pty = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let Message::Text(text) = msg {
                let _ = std::io::Write::write_all(&mut writer, text.as_bytes());
                let _ = std::io::Write::flush(&mut writer);
            }
        }
    });
    tokio::select! { _ = pty_to_ws => {}, _ = ws_to_pty => {} }
}

async fn handle_read_file(Query(params): Query<FileParams>) -> Result<Json<FileContent>, StatusCode> {
    let target = if params.path.is_empty() { "src/main.rs" } else { &params.path };
    match fs::read_to_string(target) {
        Ok(content) => Ok(Json(FileContent { path: target.to_string(), content })),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> StatusCode {
    if fs::write(&payload.path, &payload.content).is_ok() { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR }
}

async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let path = if params.path.is_empty() { "." } else { &params.path };
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(path) {
        for entry in rd.flatten() {
            let is_dir = entry.path().is_dir();
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                entries.push(DirEntry { name, is_dir, path: entry.path().to_string_lossy().to_string() });
            }
        }
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Json(entries)
}
