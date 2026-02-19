use axum::{
    body::Body,
    extract::{State, Query},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use std::fs;
use std::path::Path as StdPath; 
use std::process::Command;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::memory::MemoryPalace;
use crate::system_monitor::SystemMonitor;
use crate::vision;

// --- EMBEDDED UI ASSETS ---
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
pub struct FileParams { 
    pub path: String, 
    pub recursive: Option<bool>,
    pub start_line: Option<usize>, 
    pub end_line: Option<usize> 
}

#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String }
#[derive(Deserialize, Serialize)] pub struct PromptRequest { 
    pub prompt: String, 
    pub context: Option<String>, 
    pub terminal_context: Option<String>,
    pub image: Option<String>,
    pub use_vision: Option<bool>
}
#[derive(Serialize)] pub struct NeuralResponse { pub response: String, pub status: String }

#[derive(Serialize)]
pub struct VitalsResponse {
    pub status: String,
    pub cpu_usage: f32,
    pub ram_usage: String,
}

// --- UTILS ---

fn read_dir_recursive(path: &StdPath, entries: &mut Vec<DirEntry>) {
    if let Ok(rd) = fs::read_dir(path) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = p.is_dir();
            
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == "dist" || name == "ui" { continue; }
            
            entries.push(DirEntry { name: name.clone(), is_dir, path: p.to_string_lossy().to_string() });
            if is_dir { read_dir_recursive(&p, entries); }
        }
    }
}

fn generate_project_map() -> String {
    let mut entries = Vec::new();
    read_dir_recursive(StdPath::new("src"), &mut entries);
    entries.iter().filter(|e| !e.is_dir).map(|e| e.path.clone()).collect::<Vec<_>>().join("\n")
}

// --- SERVER INITIALIZATION ---

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/vitals", get(handle_vitals))
        .route("/api/vision/frame", get(handle_vision_frame))
        .route("/api/prompt", post(handle_prompt))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file)) 
        .route("/api/fs/list", get(handle_list_files))
        .route("/api/memories", get(handle_get_memories))
        .route("/api/memories/purge", delete(handle_purge_memories))
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.expect("CRITICAL: Port 3000 is blocked.");
    println!("\n>> [NEXUS] INDUSTRIAL ENGINE ONLINE: http://localhost:3000\n");
    axum::serve(listener, app).await.unwrap();
}

async fn index_handler() -> impl IntoResponse {
    static_handler(Uri::from_static("/index.html")).await
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() { path = "index.html".to_string(); }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        },
        None => {
            if let Some(index) = Assets::get("index.html") {
                return ([(header::CONTENT_TYPE, "text/html")], index.data).into_response();
            }
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<VitalsResponse> {
    let snapshot = state.monitor.get_vitals();
    Json(VitalsResponse {
        status: snapshot.status,
        cpu_usage: snapshot.cpu_load * 10.0, 
        ram_usage: format!("{:.2}GB / {:.2}GB", snapshot.ram_used_gb, snapshot.ram_total_gb),
    })
}

async fn handle_get_memories(State(state): State<Arc<AppState>>) -> Json<Vec<crate::memory::Memory>> {
    Json(state.memory.load_all())
}

async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> StatusCode {
    state.memory.purge();
    StatusCode::OK
}

async fn handle_read_file(Query(params): Query<FileParams>) -> Result<Json<FileContent>, StatusCode> {
    let target = if params.path.is_empty() { "src/main.rs" } else { &params.path };
    let full_content = fs::read_to_string(target).map_err(|_| StatusCode::NOT_FOUND)?;
    
    let content = if let (Some(s), Some(e)) = (params.start_line, params.end_line) {
        full_content.lines()
            .skip(s.saturating_sub(1))
            .take(e.saturating_sub(s).saturating_add(1))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        full_content
    };

    Ok(Json(FileContent { path: target.into(), content }))
}

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    let vitals = state.monitor.get_vital_signs_string();
    let project_map = generate_project_map();
    
    let vision_data = if payload.use_vision.unwrap_or(false) {
        vision::capture_frame().await.ok()
    } else {
        None
    };

    let processed_context = payload.context.map(|c| {
        let lines: Vec<&str> = c.lines().collect();
        if lines.len() > 300 {
            let head = lines[..50].join("\n");
            let tail = lines[lines.len().saturating_sub(100)..].join("\n");
            format!("{}\n\n// ... [LOGIC GAP: {} LINES] ...\n\n{}", head, lines.len().saturating_sub(150), tail)
        } else {
            c
        }
    });

    let packet = ContextPacket {
        file_content: processed_context,
        terminal_log: payload.terminal_context.or(Some(project_map)), 
        memory_recall: Some(state.memory.recall()),
        system_stats: Some(vitals),
        image_b64: vision_data,
    };

    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, packet).await {
        Ok((res, duration)) => {
            if res.contains("```") {
                state.memory.remember(payload.prompt, res.clone(), "SUCCESS".into(), None);
            }
            Json(NeuralResponse { response: res, status: format!("{}ms", duration) })
        },
        Err(e) => Json(NeuralResponse { response: format!("AI ERROR: {}", e), status: "fail".into() }),
    }
}

async fn handle_vision_frame() -> Json<NeuralResponse> {
    match vision::capture_frame().await {
        Ok(frame) => Json(NeuralResponse { response: frame, status: "ok".into() }),
        Err(e) => Json(NeuralResponse { response: format!("Capture Error: {}", e), status: "fail".into() }),
    }
}

async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let path_str = if params.path.is_empty() { "." } else { &params.path };
    let path = StdPath::new(path_str);
    let mut entries = Vec::new();
    
    if params.recursive.unwrap_or(false) { 
        read_dir_recursive(path, &mut entries); 
    } else {
        if let Ok(rd) = fs::read_dir(path) {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "target" && name != "node_modules" && name != "ui" {
                    entries.push(DirEntry { name, is_dir: entry.path().is_dir(), path: entry.path().to_string_lossy().into() });
                }
            }
        }
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Json(entries)
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> StatusCode {
    let clean_content = if payload.content.trim().starts_with("FILE:") {
        payload.content.lines().skip(1).collect::<Vec<_>>().join("\n")
    } else {
        payload.content
    };
    
    if payload.path.contains("..") || payload.path.starts_with("/") {
        return StatusCode::FORBIDDEN;
    }

    if fs::write(&payload.path, &clean_content).is_ok() { 
        if payload.path.ends_with(".rs") {
             let _ = Command::new("rustfmt").arg(&payload.path).output();
        }
        StatusCode::OK 
    } else { 
        StatusCode::INTERNAL_SERVER_ERROR 
    }
}
