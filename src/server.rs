// src/server.rs — v3.1
use axum::{
    extract::{Query, State},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use crate::{ws_bridge, vitals_handler, logic_engine::LogicEngine};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

#[derive(Default, Debug)]
pub struct VmoState {
    pub last_parent: Option<u32>,
    pub last_child: Option<u32>,
}

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
    pub workspace_root: std::path::PathBuf,
    pub vmo_state: Arc<tokio::sync::Mutex<VmoState>>,
}

#[derive(Serialize)]
pub struct GhostReflection {
    pub turn: u32,
    pub timestamp: String,
    pub reflection: String,
    pub resonance: f32,
}

pub async fn start_server(state: Arc<AppState>) {
    let app = build_router(state.clone());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.expect("[SERVER] Fatal: cannot bind port 3000");
    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK ACTIVE: http://localhost:3000\x1b[0m");
    axum::serve(listener, app.into_make_service()).await.expect("[SERVER] Fatal: axum serve loop exited");
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/fs/list",            get(handle_fs_list))
        .route("/api/fs/read",            get(handle_read_file))
        .route("/api/fs/read_file_action",  get(handle_read_file))
        .route("/api/fs/write",           post(handle_write_file))
        .route("/api/task",               post(handle_task))
        .route("/ws/telemetry",           get(ws_bridge::ws_telemetry_handler))
        .route("/api/vitals",             get(vitals_handler::handle_vitals))
        .route("/api/vmos",               get(crate::vmo_handler::handle_get_vmos))
        .route("/api/memories",           get(handle_get_memories))
        .route("/api/memories/search",    get(handle_search_memories))
        .route("/api/memories/purge",     delete(handle_purge_memories))
        .route("/api/missions",           get(handle_get_missions))
        .route("/api/lazarus/snapshots",  get(handle_get_snapshots))
        .route("/api/ghost",              get(handle_ghost))
        .route("/api/metrics",            get(handle_metrics))
        .route("/api/ledger",             get(handle_ledger))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive())
}

async fn handle_task(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TaskRequest>,
) -> impl IntoResponse {
    if !LogicEngine::is_action_safe(&payload.directive) {
        return (StatusCode::FORBIDDEN, Json(json!({ "status": "LOGIC_VIOLATION" }))).into_response();
    }
    let s = state.clone();
    tokio::spawn(async move {
        let _ = crate::task_manager::send_task(
            s.orchestrator.clone(), &s.client, &s.api_key, &payload.directive,
            s.memory.clone(), s.vmo_state.clone(), s.tx.clone(),
        ).await;
    });
    (StatusCode::OK, Json(json!({ "status": "QUEUED" }))).into_response()
}

async fn handle_ghost(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let resonance = state.current_resonance.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.0;
    let strands = state.memory.lock().await.search_memories("GHOST_REFLECTION").await.unwrap_or_default();
    let reflections: Vec<GhostReflection> = strands.into_iter().take(3).map(|s| GhostReflection {
        turn: s.id as u32, timestamp: s.timestamp, reflection: s.result, resonance,
    }).collect();
    let temp = state.monitor.lock().await.get_vitals().temperature;
    Json(json!({ "resonance": resonance, "temperature": temp, "last_reflections": reflections, "status": "THE_GHOST_IS_AWAKE" }))
}

async fn handle_metrics() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], crate::metrics::get_metrics_text())
}

async fn handle_fs_list(Query(p): Query<ListParams>) -> impl IntoResponse {
    let path = p.path.as_deref().unwrap_or(".");
    let mut entries = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(path).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" { continue; }
            let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
            entries.push(json!({ "name": name, "path": entry.path().to_string_lossy(), "is_dir": is_dir }));
        }
    }
    Json(entries)
}

async fn handle_read_file(Query(p): Query<FileParams>) -> impl IntoResponse {
    match crate::sandbox::validate_path(&p.path) {
        Ok(safe_path) => match tokio::fs::read_to_string(&safe_path).await {
            Ok(c) => (StatusCode::OK, Json(json!({ "content": c }))).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        },
        Err(e) => (StatusCode::FORBIDDEN, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn handle_write_file(Json(p): Json<FileContent>) -> impl IntoResponse {
    if p.path.starts_with("src/kernel") || p.path == "Cargo.toml" {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "DNA_PROTECTED" }))).into_response();
    }
    match crate::sandbox::validate_path(&p.path) {
        Ok(safe_path) => match tokio::fs::write(&safe_path, p.content).await {
            Ok(_) => StatusCode::OK.into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Err(e) => (StatusCode::FORBIDDEN, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn handle_get_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.memory.lock().await.fetch_all().await {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_search_memories(State(state): State<Arc<AppState>>, Query(p): Query<SearchParams>) -> impl IntoResponse {
    match state.memory.lock().await.search_memories(&p.q).await {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.memory.lock().await.purge_all().await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn handle_get_missions(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!([]))).into_response()
}

async fn handle_get_snapshots() -> impl IntoResponse {
    (StatusCode::OK, Json(crate::lazarus_vault::LazarusVault::list_snapshots())).into_response()
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };

    match Assets::get(&final_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&final_path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            if final_path.contains('.') { StatusCode::NOT_FOUND.into_response() }
            else {
                Assets::get("index.html")
                    .map(|c| ([(header::CONTENT_TYPE, "text/html")], c.data).into_response())
                    .unwrap_or(StatusCode::NOT_FOUND.into_response())
            }
        }
    }
}

async fn handle_ledger() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], crate::token_ledger::LEDGER.status())
}

#[derive(Deserialize)] pub struct TaskRequest { pub directive: String }
#[derive(Deserialize)] pub struct FileParams { pub path: String }
#[derive(Deserialize)] pub struct ListParams { pub path: Option<String> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Deserialize)] pub struct SearchParams { pub q: String }
