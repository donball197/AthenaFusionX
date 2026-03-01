use axum::{
    extract::{State, Query, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router, body::Bytes,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use tokio::sync::Mutex; 
use std::sync::atomic::{AtomicU32, Ordering};
use std::fs;
use std::path::Path;
use tokio::process::Command;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use std::process::Command as StdCommand;

use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::memory::MemoryPalace;
use crate::system_monitor::SystemMonitor;
use crate::hive_mind::HiveMind;
use crate::broker::KnowledgeBroker;
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
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub api_key: String,
    pub hive: Option<HiveMind>,
    pub current_resonance: Arc<AtomicU32>,
    pub hiram: Arc<Mutex<Hiram>>,
    pub last_health_error: Arc<Mutex<Option<String>>>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct PromptRequest { 
    #[serde(default)] pub prompt: String, 
    pub context: Option<String>,
    pub terminal_context: Option<String>,
    pub image: Option<String>,
    pub use_vision: Option<bool>
}

#[derive(Serialize)] pub struct NeuralResponse { pub response: String, pub status: String }
#[derive(Serialize)] pub struct VitalsResponse { pub status: String, pub cpu_usage: f32, pub ram_usage: String, pub resonance: f32 }
#[derive(Deserialize)] pub struct FileParams { pub path: String, pub recursive: Option<bool> }
#[derive(Serialize, Deserialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String, pub children: Vec<DirEntry> }
#[derive(Deserialize)] pub struct BuildReq { pub path: String, pub content: String }

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/prompt", post(handle_prompt))
        .route("/api/vitals", get(handle_vitals))
        .route("/api/fs/list", get(handle_list_files))
        .route("/api/fs/tree", get(handle_file_tree))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file))
        .route("/api/devops/build", post(handle_build))
        .route("/ws", get(ws_shell_handler))
        .route("/ws/ghost", get(ws_ghost_handler))
        .route("/", get(index_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\n>> [NEXUS] CONTROL ONLINE: http://localhost:3000\n");
    axum::serve(listener, app).await.unwrap();
}

// UPGRADED: The Build Endpoint now triggers the AutoHealer natively.
async fn handle_build(State(state): State<Arc<AppState>>, Json(payload): Json<BuildReq>) -> Json<crate::polyglot::BuildResult> {
    let mut build_res = crate::polyglot::detect_and_build(&payload.path);
    
    // If the build failed, trigger the backend AutoHealer
    if !build_res.success && build_res.detected_stack == "RUST" {
        // Use the active tab file sent from the UI, or default to main.rs
        let active_file = if !payload.content.is_empty() { &payload.content } else { "src/main.rs" };
        
        if let Ok((healed, report)) = crate::auto_healer::AutoHealer::attempt_auto_repair(&state, active_file, &build_res.log).await {
            build_res.log.push_str(&report);
            if healed {
                build_res.log.push_str("\n>> SYSTEM HEALED. REFRESHING UI...");
            }
        }
    }
    Json(build_res)
}

async fn handle_prompt(State(state): State<Arc<AppState>>, body: Bytes) -> Json<NeuralResponse> {
    let payload: PromptRequest = serde_json::from_slice(&body).unwrap_or_else(|_| {
        PromptRequest { prompt: String::from_utf8_lossy(&body).to_string(), ..Default::default() }
    });

    if payload.prompt.trim().to_lowercase() == "reindex" {
        return match KnowledgeBroker::deep_index(state.clone()).await {
            Ok(msg) => Json(NeuralResponse { response: msg, status: "indexed".into() }),
            Err(e) => Json(NeuralResponse { response: format!("Fail: {}", e), status: "fail".into() }),
        };
    }

    let vitals = state.monitor.get_vital_signs_string();
    let resonance = atomic_to_f32(state.current_resonance.load(Ordering::Relaxed));
    
    let context_packet = ContextPacket { 
        system_stats: Some(vitals), 
        file_content: payload.context.clone(),
        terminal_log: payload.terminal_context.clone(),
        ..Default::default() 
    };
    
    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, context_packet, resonance).await {
        Ok((r, d)) => Json(NeuralResponse { response: r, status: format!("{}ms", d) }),
        Err(e) => Json(NeuralResponse { response: format!("Backend Error: {}", e), status: "fail".into() })
    }
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<VitalsResponse> {
    let v = state.monitor.get_vitals();
    Json(VitalsResponse { status: v.status, cpu_usage: v.cpu_load, ram_usage: format!("{:.2}GB", v.ram_used_gb), resonance: 0.98 })
}

fn build_tree(path: &Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(path) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == "dist" { continue; }
            let is_dir = p.is_dir();
            let children = if is_dir { build_tree(&p) } else { Vec::new() };
            entries.push(DirEntry { name, is_dir, path: p.to_string_lossy().to_string(), children });
        }
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    entries
}

async fn handle_file_tree() -> Json<Vec<DirEntry>> { Json(build_tree(Path::new("."))) }
async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let target = if params.path.is_empty() { "." } else { &params.path };
    Json(build_tree(Path::new(target)))
}
async fn handle_read_file(Query(params): Query<FileParams>) -> impl IntoResponse {
    match fs::read_to_string(&params.path) {
        Ok(c) => (StatusCode::OK, Json(FileContent { path: params.path, content: c })).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn handle_write_file(Json(payload): Json<FileContent>) -> impl IntoResponse {
    if let Some(parent) = Path::new(&payload.path).parent() { let _ = fs::create_dir_all(parent); }
    match fs::write(&payload.path, &payload.content) {
        Ok(_) => {
            if payload.path.ends_with(".rs") { let _ = StdCommand::new("rustfmt").arg(&payload.path).output(); }
            StatusCode::OK.into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn ws_shell_handler(ws: WebSocketUpgrade) -> impl IntoResponse { ws.on_upgrade(handle_shell_socket) }
async fn handle_shell_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(cmd) = msg {
            let _ = socket.send(Message::Text(format!("$ {}\n", cmd))).await;
            let output = Command::new("bash").arg("-c").arg(&cmd).output().await;
            match output {
                Ok(out) => {
                    let combined = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                    let _ = socket.send(Message::Text(combined)).await;
                },
                Err(e) => { let _ = socket.send(Message::Text(format!("Execution Fail: {}\n", e))).await; }
            }
        }
    }
}

async fn ws_ghost_handler(ws: WebSocketUpgrade) -> impl IntoResponse { ws.on_upgrade(handle_ghost_socket) }
async fn handle_ghost_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(t) = msg { let _ = socket.send(Message::Text(format!("Echo: {}", t))).await; }
    }
}

async fn index_handler() -> impl IntoResponse { static_handler(Uri::from_static("/index.html")).await }
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() { path = "index.html".to_string(); }
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        },
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}
