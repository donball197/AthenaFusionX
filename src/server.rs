use axum::{
    extract::{State, Query, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
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

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor,
    pub agent: MicroAgent,
    pub memory: MemoryPalace,
    pub api_key: String,
}

#[derive(Deserialize)] 
pub struct FileParams { pub path: String, pub recursive: Option<bool>, pub start_line: Option<usize>, pub end_line: Option<usize> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String }
#[derive(Deserialize, Serialize)] pub struct PromptRequest { pub prompt: String, pub context: Option<String>, pub terminal_context: Option<String>, pub image: Option<String>, pub use_vision: Option<bool> }
#[derive(Serialize)] pub struct NeuralResponse { pub response: String, pub status: String }
#[derive(Serialize)] pub struct VitalsResponse { pub status: String, pub cpu_usage: f32, pub ram_usage: String }
#[derive(Deserialize)] pub struct DeployReq { pub image: String, pub port: String }
#[derive(Deserialize)] pub struct NukeReq { pub id: String }

fn read_dir_recursive(path: &StdPath, entries: &mut Vec<DirEntry>) {
    if let Ok(rd) = fs::read_dir(path) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == "dist" || name == "ui" { continue; }
            entries.push(DirEntry { name: name.clone(), is_dir: p.is_dir(), path: p.to_string_lossy().to_string() });
            if p.is_dir() { read_dir_recursive(&p, entries); }
        }
    }
}

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/vitals", get(handle_vitals))
        .route("/api/vision/frame", get(handle_vision_frame))
        .route("/api/prompt", post(handle_prompt))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file)) 
        .route("/api/fs/list", get(handle_list_files))
        .route("/api/memories", get(handle_get_memories))
        .route("/api/memories/purge", delete(handle_purge_memories))
        .route("/api/devops/build", post(handle_build))
        .route("/api/devops/containers", get(handle_containers))
        .route("/api/devops/deploy", post(handle_deploy))
        .route("/api/devops/nuke", post(handle_nuke))
        .route("/", get(index_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Port blocked");
    println!("\n>> [NEXUS] DEVOPS ENGINE ONLINE: http://localhost:3000\n");
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse { ws.on_upgrade(handle_socket) }
async fn handle_socket(socket: WebSocket) {
    let mut child = Command::new("bash").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().expect("Fail bash");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = tokio::io::BufReader::new(child.stdout.take().unwrap());
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let _ = stdin.write_all(text.as_bytes()).await; let _ = stdin.flush().await;
        }
    });

    let mut recv_task = tokio::spawn(async move {
        let mut buffer = [0u8; 1024];
        while let Ok(n) = stdout.read(&mut buffer).await {
            if n == 0 { break; }
            let _ = sender.send(Message::Text(String::from_utf8_lossy(&buffer[..n]).into())).await;
        }
    });

    tokio::select! { _ = (&mut send_task) => recv_task.abort(), _ = (&mut recv_task) => send_task.abort(), };
}

async fn index_handler() -> impl IntoResponse { static_handler(Uri::from_static("/index.html")).await }
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() { path = "index.html".to_string(); }
    match Assets::get(&path) {
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&path).first_or_octet_stream().as_ref())], content.data).into_response(),
        None => Assets::get("index.html").map(|i| ([(header::CONTENT_TYPE, "text/html")], i.data).into_response()).unwrap_or(StatusCode::NOT_FOUND.into_response())
    }
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<VitalsResponse> {
    let snapshot = state.monitor.get_vitals();
    Json(VitalsResponse { status: snapshot.status, cpu_usage: snapshot.cpu_load * 10.0, ram_usage: format!("{:.2}GB / {:.2}GB", snapshot.ram_used_gb, snapshot.ram_total_gb) })
}
async fn handle_get_memories(State(state): State<Arc<AppState>>) -> Json<Vec<crate::memory::Memory>> { Json(state.memory.load_all()) }
async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> StatusCode { state.memory.purge(); StatusCode::OK }
async fn handle_read_file(Query(params): Query<FileParams>) -> Result<Json<FileContent>, StatusCode> {
    let target = if params.path.is_empty() { "src/main.rs" } else { &params.path };
    let full = fs::read_to_string(target).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(FileContent { path: target.into(), content: full }))
}

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    let vitals = state.monitor.get_vital_signs_string();
    let packet = ContextPacket {
        file_content: payload.context, terminal_log: payload.terminal_context, memory_recall: Some(state.memory.recall()),
        system_stats: Some(vitals), image_b64: if payload.use_vision.unwrap_or(false) { vision::capture_frame().await.ok() } else { None },
    };
    
    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, packet).await {
        Ok((res, duration)) => {
            if res.contains("```") { state.memory.remember(payload.prompt, res.clone(), "SUCCESS".into(), None); }
            Json(NeuralResponse { response: res, status: format!("{}ms", duration) })
        },
        Err(e) => Json(NeuralResponse { response: format!("AI ERROR: {}", e), status: "fail".into() }),
    }
}

async fn handle_vision_frame() -> Json<NeuralResponse> {
    match vision::capture_frame().await { Ok(frame) => Json(NeuralResponse { response: frame, status: "ok".into() }), Err(e) => Json(NeuralResponse { response: format!("Capture Error: {}", e), status: "fail".into() }) }
}

async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let path_str = if params.path.is_empty() { "." } else { &params.path };
    let mut entries = Vec::new();
    if params.recursive.unwrap_or(false) { read_dir_recursive(StdPath::new(path_str), &mut entries); }
    else if let Ok(rd) = fs::read_dir(path_str) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') && name != "target" && name != "node_modules" && name != "ui" {
                entries.push(DirEntry { name, is_dir: entry.path().is_dir(), path: entry.path().to_string_lossy().into() });
            }
        }
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Json(entries)
}

async fn handle_build(Json(payload): Json<FileContent>) -> Json<polyglot::BuildResult> { Json(polyglot::detect_and_build(&payload.path)) }
async fn handle_containers() -> Json<Vec<crate::podman::Container>> { Json(PodmanEngine::list()) }
async fn handle_deploy(Json(req): Json<DeployReq>) -> Json<String> { match PodmanEngine::deploy(&req.image, &req.port) { Ok(id) => Json(id), Err(e) => Json(format!("Deploy Error: {}", e)) } }

async fn handle_nuke(Json(req): Json<NukeReq>) -> Json<String> {
    match PodmanEngine::nuke(&req.id) {
        Ok(_) => Json("Container Nuked".to_string()),
        Err(e) => Json(format!("Nuke Error: {}", e)),
    }
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> StatusCode {
    if let Err(_e) = crate::auditor::SyntaxSentinel::validate_safety(&payload.path, &payload.content) { return StatusCode::BAD_REQUEST; }
    let clean = if payload.content.trim().starts_with("FILE:") { payload.content.lines().skip(1).collect::<Vec<_>>().join("\n") } else { payload.content };
    if payload.path.contains("..") || payload.path.starts_with("/") { return StatusCode::FORBIDDEN; }
    let path = StdPath::new(&payload.path);
    if let Some(parent) = path.parent() { if !parent.as_os_str().is_empty() { let _ = fs::create_dir_all(parent); } }
    if fs::write(&payload.path, &clean).is_ok() {
        if payload.path.ends_with(".rs") { let _ = Command::new("rustfmt").arg(&payload.path).output().await; }
        StatusCode::OK
    } else { StatusCode::INTERNAL_SERVER_ERROR }
}
