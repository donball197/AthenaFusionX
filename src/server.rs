use axum::{
    extract::{Query, State, ws::{WebSocketUpgrade, WebSocket, Message}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use crate::{task_manager, sandbox};

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
        .route("/api/fs/delete", delete(handle_delete_file))
        .route("/api/memories", get(handle_get_memories))
        .route("/api/memories/search", get(handle_search_memories))
        .route("/api/memories/purge", delete(handle_purge_memories))
        .route("/api/missions", get(handle_get_missions))
        .route("/api/lazarus/snapshots", get(handle_lazarus_snapshots))
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

async fn handle_fs_list(Query(p): Query<ListParams>, State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let path = p.path.as_deref().unwrap_or(".");
    match sandbox::validate_path(path) {
        Ok(safe) => {
            let mut entries = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&safe) {
                for entry in rd.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && name != "target" && name != "node_modules" {
                        entries.push(FsEntry {
                            name,
                            path: entry.path().to_string_lossy().to_string(),
                            is_dir: entry.path().is_dir(),
                        });
                    }
                }
            }
            (StatusCode::OK, Json(entries)).into_response()
        }
        Err(_) => (StatusCode::FORBIDDEN, Json(json!({"error": "Access Denied"}))).into_response(),
    }
}

async fn handle_get_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let palace = state.memory.lock().await;
    match palace.fetch_all().await {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Memory Read Fault: {}", e)).into_response(),
    }
}

async fn handle_search_memories(Query(q): Query<SearchQuery>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let query = q.q.as_deref().unwrap_or("");
    let palace = state.memory.lock().await;
    match palace.search_memories(query).await {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Search Fault: {}", e)).into_response(),
    }
}

async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let palace = state.memory.lock().await;
    match palace.purge_all().await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Purge Fault: {}", e)).into_response(),
    }
}

async fn handle_get_missions() -> impl IntoResponse {
    match crate::mission_queue::MissionQueue::new(".nexus_missions.db").await {
        Ok(queue) => match queue.get_pending().await {
            Ok(m) => (StatusCode::OK, Json(m)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Queue Error: {}", e)).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Init Error: {}", e)).into_response(),
    }
}

async fn handle_lazarus_snapshots() -> impl IntoResponse {
    match crate::lazarus_vault::LazarusVault::list_snapshots() {
        Ok(snaps) => (StatusCode::OK, Json(snaps)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Vault Error: {}", e)).into_response(),
    }
}

async fn handle_task(State(state): State<Arc<AppState>>, Json(payload): Json<TaskRequest>) -> impl IntoResponse {
    let agent = state.agent.clone();
    let client = state.client.clone();
    let api_key = state.api_key.clone();
    let memory = state.memory.clone();
    let directive = payload.directive.clone();
    tokio::spawn(async move {
        let _ = task_manager::send_task(agent, &client, &api_key, &directive, memory).await;
    });
    Json(json!({ "status": "QUEUED" }))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg)).await.is_err() { break; }
            }
            _ = heartbeat.tick() => {
                let v = state.monitor.get_vitals();
                let payload = json!({ "type": "telemetry", "payload": v });
                if socket.send(Message::Text(payload.to_string())).await.is_err() { break; }
            }
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Text(text) = msg {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        if parsed["type"] == "directive" {
                            let mut cmd = parsed["payload"].as_str().unwrap_or_default().to_string();
                            
                            // === SUTURE: AUTONOMIC CONTEXT INJECTOR ===
                            let lower_cmd = cmd.to_lowercase();
                            if lower_cmd.contains("audit") || lower_cmd.contains("all the code") {
                                let _ = state.tx.send(String::from(">> [CORTEX_UPLINK] Broad audit detected. Injecting local workspace DNA..."));
                                let mut context = String::from("\n\n--- [SYSTEM WORKSPACE DNA] ---\n");
                                
                                if let Ok(entries) = std::fs::read_dir("src") {
                                    for entry in entries.flatten() {
                                        let path = entry.path();
                                        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                                            if let Ok(content) = std::fs::read_to_string(&path) {
                                                // Hard limit to prevent token explosion
                                                let preview = &content; // GEMMA-4 262K UNLOCKED
                                                context.push_str(&format!("\n// === FILE: {} ===\n{}\n", path.display(), preview));
                                            }
                                        }
                                    }
                                }
                                cmd.push_str(&context);
                            }
                            
                            let state_clone = state.clone();
                            tokio::spawn(async move {
                                let _ = task_manager::send_task(
                                    state_clone.agent.clone(),
                                    &state_clone.client,
                                    &state_clone.api_key,
                                    &cmd,
                                    state_clone.memory.clone()
                                ).await;
                            });
                        }
                    }
                }
            }
            else => break,
        }
    }
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

async fn handle_read_file(Query(p): Query<FileParams>) -> impl IntoResponse {
    match sandbox::validate_path(&p.path) {
        Ok(safe) => match std::fs::read_to_string(&safe) {
            Ok(c) => (StatusCode::OK, Json(json!({ "content": c }))).into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
        },
        Err(_) => (StatusCode::FORBIDDEN, Json(json!({"error": "Access Denied"}))).into_response(),
    }
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> impl IntoResponse {
    match sandbox::validate_path(&payload.path) {
        Ok(safe) => {
            if let Some(p) = safe.parent() { let _ = std::fs::create_dir_all(p); }
            match std::fs::write(&safe, &payload.content) {
                Ok(_) => StatusCode::OK.into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => (StatusCode::FORBIDDEN, Json(json!({"error": "Access Denied"}))).into_response(),
    }
}

async fn handle_delete_file(Query(p): Query<FileParams>) -> impl IntoResponse {
    match sandbox::validate_path(&p.path) {
        Ok(safe) => {
            let res = if safe.is_dir() { std::fs::remove_dir_all(&safe) } else { std::fs::remove_file(&safe) };
            match res {
                Ok(_) => StatusCode::OK.into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => (StatusCode::FORBIDDEN, Json(json!({"error": "Access Denied"}))).into_response(),
    }
}

#[derive(Deserialize)] pub struct TaskRequest  { pub directive: String }
#[derive(Deserialize)] pub struct FileParams   { pub path: String }
#[derive(Deserialize)] pub struct ListParams   { pub path: Option<String> }
#[derive(Deserialize)] pub struct SearchQuery  { pub q: Option<String> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)]   pub struct FsEntry      { pub name: String, pub path: String, pub is_dir: bool }
