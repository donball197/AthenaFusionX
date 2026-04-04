use axum::{
    extract::{Query, State, ws::{WebSocketUpgrade, WebSocket, Message}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use walkdir::WalkDir;
use crate::{task_manager, sandbox, nexus_error::NexusError};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client: reqwest::Client,
    pub monitor: crate::system_monitor::SystemMonitor,
    pub agent: Arc<crate::neural_agent::MicroAgent>,
    pub orchestrator: Arc<crate::orchestrator::Orchestrator>,
    pub memory: Arc<tokio::sync::Mutex<crate::memory::MemoryPalace>>,
    pub api_key: String,
    pub current_resonance: Arc<std::sync::atomic::AtomicU32>,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub scheduler_tx: tokio::sync::mpsc::Sender<crate::scheduler::NexusTask>,
    pub arbiter: Arc<crate::arbiter_agent::ArbiterAgent>,
    pub local_llm: Arc<crate::local_llm::LocalLLM>,
    pub workspace_root: std::path::PathBuf,
}

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/fs/list", get(handle_fs_list))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file))
        .route("/api/memories", get(handle_get_memories)) // NEW
        .route("/api/task", post(handle_task))
        .route("/ws/nexus", get(ws_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("\x1b[38;2;0;255;255m>> [NEXUS SERVER] UNIFIED UPLINK: http://localhost:3000\x1b[0m");
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

async fn handle_get_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let palace = state.memory.lock().await;
    match palace.fetch_all().await {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ... [Keep other handle functions from previous consolidation] ...

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => { if socket.send(Message::Text(msg)).await.is_err() { break; } }
            _ = heartbeat.tick() => {
                let v = state.monitor.get_vitals();
                let payload = serde_json::json!({ "type": "telemetry", "payload": v });
                if socket.send(Message::Text(payload.to_string())).await.is_err() { break; }
            }
            Some(res) = socket.recv() => { if res.is_err() { break; } }
        }
    }
}

async fn handle_task(State(state): State<Arc<AppState>>, Json(payload): Json<TaskRequest>) -> impl IntoResponse {
    let result = task_manager::send_task(state.agent.clone(), &state.client, &state.api_key, &payload.directive).await;
    Json(serde_json::json!({ "output": result }))
}

async fn handle_fs_list(Query(p): Query<ListParams>) -> impl IntoResponse {
    let path = p.path.as_deref().unwrap_or("src");
    if let Ok(safe) = sandbox::validate_path(path) {
        let mut entries = Vec::new();
        for entry in WalkDir::new(safe).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            entries.push(FsEntry { 
                name: entry.file_name().to_string_lossy().to_string(), 
                path: entry.path().to_string_lossy().to_string(), 
                is_dir: entry.path().is_dir() 
            });
        }
        (StatusCode::OK, Json(entries)).into_response()
    } else {
        (StatusCode::FORBIDDEN, "Access Denied").into_response()
    }
}

async fn handle_read_file(Query(p): Query<FileParams>) -> impl IntoResponse {
    if let Ok(safe) = sandbox::validate_path(&p.path) {
        if let Ok(c) = std::fs::read_to_string(safe) {
            return (StatusCode::OK, Json(serde_json::json!({ "content": c }))).into_response();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> impl IntoResponse {
    if let Ok(safe) = sandbox::validate_path(&payload.path) {
        if let Some(p) = safe.parent() { let _ = std::fs::create_dir_all(p); }
        if std::fs::write(safe, &payload.content).is_ok() { return StatusCode::OK.into_response(); }
    }
    StatusCode::FORBIDDEN.into_response()
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };
    match Assets::get(&final_path) {
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&final_path).first_or_octet_stream().as_ref())], content.data).into_response(),
        None => if !final_path.contains('.') { ([(header::CONTENT_TYPE, "text/html")], Assets::get("index.html").unwrap().data).into_response() } 
                else { StatusCode::NOT_FOUND.into_response() }
    }
}

#[derive(Deserialize)] pub struct TaskRequest { pub directive: String }
#[derive(Deserialize)] pub struct FileParams { pub path: String }
#[derive(Deserialize)] pub struct ListParams { pub path: Option<String> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct FsEntry { pub name: String, pub path: String, pub is_dir: bool }
