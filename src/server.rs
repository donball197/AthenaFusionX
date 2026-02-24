use axum::{
    extract::{State, Query, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::fs;
use std::path::Path as StdPath; 
use tokio::process::Command;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::memory::MemoryPalace;
use crate::system_monitor::SystemMonitor;
use crate::vision;
use crate::podman::PodmanEngine;
use crate::polyglot;
use crate::broker::{KnowledgeBroker, NeuralGraph};
use crate::hiram_core::Hiram;

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

fn atomic_to_f32(val: u32) -> f32 { f32::from_bits(val) }

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor,
    pub agent: MicroAgent,
    pub memory: MemoryPalace,
    pub api_key: String,
    pub hive: Option<crate::hive_mind::HiveMind>,
    pub current_resonance: Arc<AtomicU32>,
    pub hiram: Arc<tokio::sync::Mutex<Hiram>>,
    pub last_health_error: Arc<tokio::sync::Mutex<Option<String>>>,
}

#[derive(Deserialize)] pub struct FileParams { pub path: String, pub recursive: Option<bool> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String }
#[derive(Deserialize)] pub struct RenameReq { pub old_path: String, pub new_path: String }
#[derive(Deserialize, Serialize)] pub struct PromptRequest { pub prompt: String, pub context: Option<String>, pub terminal_context: Option<String>, pub image: Option<String>, pub use_vision: Option<bool> }
#[derive(Serialize)] pub struct NeuralResponse { pub response: String, pub status: String }
#[derive(Serialize)] pub struct VitalsResponse { pub status: String, pub cpu_usage: f32, pub ram_usage: String, pub resonance: f32 }
#[derive(Deserialize)] pub struct HiveReq { pub drone: String, pub action: String }
#[derive(Deserialize)] pub struct ForgeReq { pub name: String, pub dockerfile: String }
#[derive(Deserialize)] pub struct PredictReq { pub content: String, pub cursor: usize, pub path: String }
#[derive(Serialize)] pub struct PredictRes { pub prediction: String }

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/ws", get(handle_ws))
        .route("/ws/ghost", get(handle_ws))
        .route("/api/vitals", get(handle_vitals))
        .route("/api/vision/frame", get(handle_vision_frame))
        .route("/api/prompt", post(handle_prompt))
        .route("/api/predict", post(handle_predict))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file)) 
        .route("/api/fs/list", get(handle_list_files))
        .route("/api/fs/delete", delete(handle_delete_file)) // NEW
        .route("/api/fs/rename", post(handle_rename_file))   // NEW
        .route("/api/hive/status", get(handle_hive_status))
        .route("/api/hive/spawn", post(handle_hive_spawn))
        .route("/api/hive/kill_all", post(handle_hive_kill))
        .route("/api/hive/forge", post(handle_hive_forge)) 
        .route("/api/devops/build", post(handle_build))
        .route("/api/graph", get(handle_graph))
        .route("/api/memories/purge", delete(handle_purge_memories))
        .route("/", get(index_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\n>> [NEXUS: HIVE QUEEN] CONTROL ONLINE: http://localhost:3000\n");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_ws(ws: WebSocketUpgrade) -> impl IntoResponse { ws.on_upgrade(handle_socket) }

async fn handle_socket(socket: WebSocket) {
    let mut child = Command::new("bash").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().expect("Fail bash");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (mut sender, mut receiver) = socket.split();
    tokio::select! {
        _ = async {
            while let Some(Ok(Message::Text(text))) = receiver.next().await {
                if stdin.write_all(text.as_bytes()).await.is_err() { break; }
                if stdin.flush().await.is_err() { break; }
            }
        } => {},
        _ = async {
             let mut reader = tokio::io::BufReader::new(stdout);
             let mut buffer = [0u8; 1024];
             while let Ok(n) = reader.read(&mut buffer).await {
                 if n == 0 { break; }
                 if sender.send(Message::Text(String::from_utf8_lossy(&buffer[..n]).into())).await.is_err() { break; }
             }
        } => {}
    }
}

async fn handle_predict(State(state): State<Arc<AppState>>, Json(payload): Json<PredictReq>) -> Json<PredictRes> {
    let lines: Vec<&str> = payload.content.lines().collect();
    let start = lines.len().saturating_sub(20);
    let context = lines[start..].join("\n");
    let prompt = format!("COMPLETE CODE. NO MARKDOWN. JUST CODE.\nCONTEXT:\n{}", context);
    let packet = ContextPacket::default();
    let (text, _) = state.agent.prepare_and_run(&state.client, &state.api_key, &prompt, packet, 0.0).await.unwrap_or(("".into(), 0));
    let clean = text.replace("```rust", "").replace("```", "").trim().to_string();
    Json(PredictRes { prediction: clean })
}

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    let vitals = state.monitor.get_vital_signs_string();
    let resonance = atomic_to_f32(state.current_resonance.load(Ordering::Relaxed));
    let packet = ContextPacket { file_content: payload.context.clone(), terminal_log: payload.terminal_context.clone(), memory_recall: Some(state.memory.recall()), system_stats: Some(vitals), image_b64: if payload.use_vision.unwrap_or(false) { vision::capture_frame().await.ok() } else { None } };
    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, packet, resonance).await {
        Ok((r, d)) => {
            if r.contains("```") { state.memory.remember(payload.prompt.clone(), r.clone(), payload.terminal_context.unwrap_or_default(), None); }
            Json(NeuralResponse { response: r, status: format!("{}ms", d) })
        },
        Err(e) => Json(NeuralResponse { response: e.to_string(), status: "fail".into() })
    }
}

async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let path_str = if params.path.is_empty() { "." } else { &params.path };
    let mut entries = Vec::new();
    fn read_recursive(path: &StdPath, entries: &mut Vec<DirEntry>) {
        if let Ok(rd) = fs::read_dir(path) {
            for entry in rd.flatten() {
                let p = entry.path();
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "target" && name != "node_modules" && name != "ui" {
                    entries.push(DirEntry { name, is_dir: p.is_dir(), path: p.to_string_lossy().to_string() });
                    if p.is_dir() { read_recursive(&p, entries); }
                }
            }
        }
    }
    if params.recursive.unwrap_or(false) { read_recursive(StdPath::new(path_str), &mut entries); }
    else { if let Ok(rd) = fs::read_dir(path_str) { for e in rd.flatten() { let p = e.path(); entries.push(DirEntry { name: p.file_name().unwrap().to_string_lossy().to_string(), is_dir: p.is_dir(), path: p.to_string_lossy().to_string() }); } } }
    Json(entries)
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<VitalsResponse> {
    let v = state.monitor.get_vitals();
    Json(VitalsResponse { status: v.status, cpu_usage: v.cpu_load * 10.0, ram_usage: format!("{:.2}GB / {:.2}GB", v.ram_used_gb, v.ram_total_gb), resonance: atomic_to_f32(state.current_resonance.load(Ordering::Relaxed)) })
}

async fn handle_write_file(State(_state): State<Arc<AppState>>, Json(payload): Json<FileContent>) -> impl IntoResponse {
    let path = StdPath::new(&payload.path);
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    match fs::write(path, &payload.content) {
        Ok(_) => { 
            if payload.path.ends_with(".rs") { let _ = Command::new("rustfmt").arg(&payload.path).spawn(); }
            (StatusCode::OK, Json("Saved".to_string())).into_response() 
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response()
    }
}

// NEW: Handle File Deletion
async fn handle_delete_file(Query(params): Query<FileParams>) -> impl IntoResponse {
    match fs::remove_file(&params.path) {
        Ok(_) => (StatusCode::OK, Json("Deleted")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
    }
}

// NEW: Handle File Renaming
async fn handle_rename_file(Json(payload): Json<RenameReq>) -> impl IntoResponse {
    match fs::rename(&payload.old_path, &payload.new_path) {
        Ok(_) => (StatusCode::OK, Json("Renamed")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response(),
    }
}

async fn handle_build(Json(payload): Json<FileContent>) -> Json<polyglot::BuildResult> { Json(polyglot::detect_and_build(&payload.path)) }
async fn handle_read_file(Query(params): Query<FileParams>) -> impl IntoResponse { match fs::read_to_string(&params.path) { Ok(c) => (StatusCode::OK, Json(FileContent { path: params.path, content: c })).into_response(), Err(_) => StatusCode::NOT_FOUND.into_response() } }
async fn handle_hive_status() -> Json<Vec<crate::podman::Container>> { Json(PodmanEngine::list()) }
async fn handle_hive_spawn(Json(req): Json<HiveReq>) -> Json<String> { match PodmanEngine::spawn_drone(&req.drone) { Ok(id) => Json(format!("Drone Online: {}", id)), Err(e) => Json(e.to_string()) } }
async fn handle_hive_kill() -> StatusCode { let _ = PodmanEngine::kill_all(); StatusCode::OK }
async fn handle_hive_forge(Json(req): Json<ForgeReq>) -> Json<String> { match PodmanEngine::forge_drone(&req.name, &req.dockerfile) { Ok(res) => Json(res), Err(e) => Json(e.to_string()) } }
async fn handle_graph(State(state): State<Arc<AppState>>) -> Json<NeuralGraph> { Json(KnowledgeBroker::get_graph(state).await) }
async fn handle_vision_frame() -> Json<NeuralResponse> { match vision::capture_frame().await { Ok(frame) => Json(NeuralResponse { response: frame, status: "ok".into() }), Err(e) => Json(NeuralResponse { response: format!("Capture Error: {}", e), status: "fail".into() }) } }
async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> StatusCode { state.memory.purge(); StatusCode::OK }
async fn index_handler() -> impl IntoResponse { static_handler(Uri::from_static("/index.html")).await }
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string(); if path.is_empty() { path = "index.html".to_string(); }
    match Assets::get(&path) { Some(content) => { let mime = mime_guess::from_path(&path).first_or_octet_stream(); ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response() }, None => (StatusCode::NOT_FOUND, "404 Not Found").into_response() }
}
