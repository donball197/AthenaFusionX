use axum::{
    extract::{State, Query},
    routing::{get, post, delete},
    Json, Router, http::StatusCode,
};
use std::sync::Arc;
use std::fs;
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::memory::MemoryPalace;
use crate::system_monitor::SystemMonitor;
use crate::vision;

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor,
    pub agent: MicroAgent,
    pub memory: MemoryPalace,
    pub api_key: String,
}

#[derive(Deserialize)] pub struct FileParams { 
    pub path: String, 
    pub recursive: Option<bool>,
    pub start_line: Option<usize>, 
    pub end_line: Option<usize> 
}

#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String }
#[derive(Deserialize, Serialize)] pub struct PromptRequest { pub prompt: String, pub context: Option<String>, pub image: Option<String> }
#[derive(Serialize)] pub struct NeuralResponse { pub response: String, pub status: String }

#[derive(Serialize)]
pub struct VitalsResponse {
    pub status: String,
    pub cpu_usage: f32,
    pub ram_usage: String,
}

fn read_dir_recursive(path: &Path, entries: &mut Vec<DirEntry>) {
    if let Ok(rd) = fs::read_dir(path) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = p.is_dir();
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == "dist" { continue; }
            entries.push(DirEntry { name: name.clone(), is_dir, path: p.to_string_lossy().to_string() });
            if is_dir { read_dir_recursive(&p, entries); }
        }
    }
}

fn generate_project_map() -> String {
    let mut entries = Vec::new();
    read_dir_recursive(Path::new("src"), &mut entries);
    read_dir_recursive(Path::new("ui/src"), &mut entries);
    entries.iter().filter(|e| !e.is_dir).map(|e| e.path.clone()).collect::<Vec<_>>().join("\n")
}

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/vitals", get(handle_vitals))
        .route("/api/vision/frame", post(handle_vision_frame))
        .route("/api/prompt", post(handle_prompt))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file)) 
        .route("/api/fs/list", get(handle_list_files))
        .route("/api/fs/check", post(handle_compile_check)) 
        .fallback_service(ServeDir::new("ui/dist"))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.expect("CRITICAL: Port 3000 is blocked.");
    println!("\n>> [NEXUS] INDUSTRIAL ENGINE ONLINE: http://localhost:3000\n");
    axum::serve(listener, app).await.unwrap();
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
    let vitals = state.monitor.get_vital_signs();
    let project_map = generate_project_map();
    
    let processed_context = payload.context.map(|c| {
        let lines: Vec<&str> = c.lines().collect();
        if lines.len() > 300 {
            let head = lines[..50].join("\n");
            let tail = lines[lines.len().saturating_sub(100)..].join("\n");
            format!("{}\n\n// ... [LOGIC GAP: {} LINES HIDDEN] ...\n\n{}", head, lines.len().saturating_sub(150), tail)
        } else {
            c
        }
    });

    let packet = ContextPacket {
        file_content: processed_context,
        terminal_log: Some(project_map), 
        memory_recall: Some(state.memory.recall()),
        system_stats: Some(vitals),
        image_b64: payload.image,
    };
    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, packet).await {
        Ok((res, _)) => Json(NeuralResponse { response: res, status: "ok".into() }),
        Err(e) => Json(NeuralResponse { response: format!("AI ERROR: {}", e), status: "fail".into() }),
    }
}

async fn handle_vision_frame() -> Json<NeuralResponse> {
    match vision::capture_frame().await {
        Ok(frame) => Json(NeuralResponse { response: frame, status: "ok".into() }),
        Err(e) => Json(NeuralResponse { response: format!("Capture Error: {}", e), status: "fail".into() }),
    }
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<VitalsResponse> {
    let vitals_raw = state.monitor.get_vital_signs();
    Json(VitalsResponse {
        status: "ONLINE".into(),
        cpu_usage: 0.0,
        ram_usage: vitals_raw,
    })
}

async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let path_str = if params.path.is_empty() { "." } else { &params.path };
    let path = Path::new(path_str);
    let mut entries = Vec::new();
    if params.recursive.unwrap_or(false) { read_dir_recursive(path, &mut entries); }
    else {
        if let Ok(rd) = fs::read_dir(path) {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
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
    if fs::write(&payload.path, &clean_content).is_ok() { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR }
}

async fn handle_compile_check(Json(payload): Json<FileContent>) -> Json<NeuralResponse> {
    let clean_content = if payload.content.trim().starts_with("FILE:") {
        payload.content.lines().skip(1).collect::<Vec<_>>().join("\n")
    } else {
        payload.content
    };
    
    let original_content = fs::read_to_string(&payload.path).unwrap_or_default();
    if fs::write(&payload.path, &clean_content).is_err() {
        return Json(NeuralResponse { response: "Write failure".into(), status: "error".into() });
    }

    let output = Command::new("cargo").arg("check").output();
    let _ = fs::write(&payload.path, original_content);
    
    match output {
        Ok(out) => {
            if out.status.success() { 
                Json(NeuralResponse { response: "PROJECT INTEGRITY OK".into(), status: "ok".into() }) 
            } else { 
                Json(NeuralResponse { 
                    response: String::from_utf8_lossy(&out.stderr).to_string(), 
                    status: "fail".into() 
                }) 
            }
        },
        Err(_) => Json(NeuralResponse { response: "CARGO NOT FOUND".into(), status: "error".into() })
    }
}
