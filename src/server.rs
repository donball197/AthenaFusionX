use crate::task_manager;
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Query, State},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client: reqwest::Client,
    pub monitor: Mutex<crate::system_monitor::SystemMonitor>,
    pub agent: Arc<crate::neural_agent::MicroAgent>,
    pub orchestrator: Arc<crate::orchestrator::Orchestrator>,
    pub memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    pub api_key: String,
    pub current_resonance: Arc<std::sync::atomic::AtomicU32>,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub scheduler_tx: tokio::sync::mpsc::Sender<crate::scheduler::NexusTask>,
    pub arbiter: Arc<crate::arbiter_agent::ArbiterAgent>,
    pub local_llm: Arc<crate::local_llm::LocalLLM>,
    pub workspace_root: std::path::PathBuf,
}

pub async fn start_server(state: Arc<AppState>) {
    let app = build_router(state.clone());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK ACTIVE: http://localhost:3000\x1b[0m");
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/fs/list", get(handle_fs_list))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file))
        .route("/api/memories", get(handle_get_memories))
        .route("/api/task", post(handle_task))
        .route("/ws/nexus", get(ws_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => { 
                let json_msg = json!({"type": "NEURAL_STREAM", "payload": {"text": msg}}).to_string();
                if socket.send(Message::Text(json_msg.into())).await.is_err() { break; } 
            }
            _ = heartbeat.tick() => {
                let mut monitor = state.monitor.lock().await;
                let v = monitor.get_vitals();
                let payload = json!({"type": "telemetry", "payload": {
                    "temperature": v.temperature, 
                    "cpu_load": v.cpu_load, 
                    "ram_gb": v.ram_gb,
                    "total_ram_gb": v.total_ram_gb
                }});
                if socket.send(Message::Text(payload.to_string().into())).await.is_err() { break; }
            }
        }
    }
}

async fn handle_fs_list(Query(p): Query<ListParams>) -> impl IntoResponse { 
    let path = p.path.as_deref().unwrap_or("."); 
    let mut entries = Vec::new(); 
    if let Ok(rd) = std::fs::read_dir(path) { 
        for entry in rd.flatten() { 
            let name = entry.file_name().to_string_lossy().to_string(); 
            if !name.starts_with('.') && name != "target" { 
                entries.push(json!({ "name": name, "path": entry.path().to_string_lossy(), "is_dir": entry.path().is_dir() })); 
            } 
        } 
    } 
    Json(entries) 
}

async fn handle_read_file(Query(p): Query<FileParams>) -> impl IntoResponse { 
    match std::fs::read_to_string(&p.path) { 
        Ok(c) => (StatusCode::OK, Json(json!({ "content": c }))).into_response(), 
        Err(_) => StatusCode::NOT_FOUND.into_response(), 
    } 
}

async fn handle_task(State(state): State<Arc<AppState>>, Json(payload): Json<TaskRequest>) -> impl IntoResponse { 
    let s = state.clone(); 
    tokio::spawn(async move { 
        task_manager::send_task(s.agent.clone(), &s.client, &s.api_key, &payload.directive, s.memory.clone(), s.tx.clone()).await; 
    }); 
    Json(json!({ "status": "QUEUED" })) 
}

async fn handle_get_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse { 
    let palace = state.memory.lock().await; 
    match palace.fetch_all().await { 
        Ok(m) => (StatusCode::OK, Json(m)).into_response(), 
        Err(_) => StatusCode::NOT_FOUND.into_response() 
    } 
}

async fn static_handler(uri: Uri) -> impl IntoResponse { 
    let path = uri.path().trim_start_matches('/').to_string(); 
    let final_path = if path.is_empty() { "index.html".to_string() } else { path }; 
    match Assets::get(&final_path) { 
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&final_path).first_or_octet_stream().as_ref())], content.data).into_response(), 
        None => Assets::get("index.html").map(|c| ([(header::CONTENT_TYPE, "text/html")], c.data).into_response()).unwrap_or(StatusCode::NOT_FOUND.into_response()) 
    } 
}

// ── ARMORED WRITE HANDLER ──────────────────────────────────────────────
async fn handle_write_file(Json(p): Json<FileContent>) -> impl IntoResponse { 
    if p.path.starts_with("src/") || p.path.starts_with("lib/") || p.path == "Cargo.toml" {
        return StatusCode::FORBIDDEN;
    }
    match std::fs::write(&p.path, p.content) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Deserialize)] pub struct TaskRequest { pub directive: String }
#[derive(Deserialize)] pub struct FileParams { pub path: String }
#[derive(Deserialize)] pub struct ListParams { pub path: Option<String> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
