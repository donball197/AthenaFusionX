/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


use axum::{
    extract::{State, Query, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicU32, Ordering};
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
use crate::hive_mind::HiveMind;
use crate::broker::{KnowledgeBroker, NeuralGraph};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

static DISSECTION_PROGRESS: AtomicUsize = AtomicUsize::new(0);

fn atomic_to_f32(val: u32) -> f32 { f32::from_bits(val) }

pub struct AppState {
    pub client: reqwest::Client,
    pub brain_endpoint: String,
    pub monitor: SystemMonitor,
    pub agent: MicroAgent,
    pub memory: MemoryPalace,
    pub api_key: String,
    pub hive: Option<HiveMind>,
    pub current_resonance: Arc<AtomicU32>,
}

#[derive(Deserialize)] pub struct FileParams { pub path: String, pub recursive: Option<bool> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct DirEntry { pub name: String, pub is_dir: bool, pub path: String }
#[derive(Deserialize, Serialize)] pub struct PromptRequest { pub prompt: String, pub context: Option<String>, pub terminal_context: Option<String>, pub image: Option<String>, pub use_vision: Option<bool> }
#[derive(Serialize)] pub struct NeuralResponse { pub response: String, pub status: String }
#[derive(Serialize)] pub struct VitalsResponse { pub status: String, pub cpu_usage: f32, pub ram_usage: String, pub resonance: f32 }
#[derive(Deserialize)] pub struct HiveReq { pub drone: String, pub action: String }
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
        .route("/api/hive/status", get(handle_hive_status))
        .route("/api/hive/spawn", post(handle_hive_spawn))
        .route("/api/hive/kill_all", post(handle_hive_kill))
        .route("/api/devops/build", post(handle_build))
        .route("/api/devops/deploy", post(handle_deploy))
        .route("/api/devops/nuke", post(handle_nuke))
        .route("/api/system/cleanup", post(handle_cleanup))
        .route("/api/dissect/progress", get(handle_dissect_progress))
        .route("/api/graph", get(handle_graph))
        .route("/", get(index_handler))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\n>> [NEXUS: HIVE QUEEN] CONTROL ONLINE: http://localhost:3000\n");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_dissect_progress() -> Json<usize> { Json(DISSECTION_PROGRESS.load(Ordering::SeqCst)) }

async fn perform_dissection(path_str: &str) -> String {
    use std::path::Path;
    let filename = Path::new(path_str).file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
    DISSECTION_PROGRESS.store(5, Ordering::SeqCst);
    let _ = Command::new("podman").args(&["cp", path_str, &format!("nexus_secops:/tmp/{}", filename)]).output().await;
    DISSECTION_PROGRESS.store(50, Ordering::SeqCst);
    let info_out = Command::new("podman").args(&["exec", "nexus_secops", "rabin2", "-I", &format!("/tmp/{}", filename)]).output().await.ok();
    let info_str = info_out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or("Analysis failed.".into());
    DISSECTION_PROGRESS.store(100, Ordering::SeqCst);
    format!("=== DISSECTION OF {} ===\n\n{}", filename, info_str)
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse { ws.on_upgrade(handle_socket) }
async fn handle_socket(socket: WebSocket) {
    let mut child = Command::new("bash").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().expect("Fail bash");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = tokio::io::BufReader::new(child.stdout.take().unwrap());
    let (mut sender, mut receiver) = socket.split();
    let mut send_task = tokio::spawn(async move { while let Some(Ok(Message::Text(text))) = receiver.next().await { let _ = stdin.write_all(text.as_bytes()).await; let _ = stdin.flush().await; } });
    let mut recv_task = tokio::spawn(async move { let mut buffer = [0u8; 1024]; while let Ok(n) = stdout.read(&mut buffer).await { if n == 0 { break; } let _ = sender.send(Message::Text(String::from_utf8_lossy(&buffer[..n]).into())).await; } });
    tokio::select! { _ = (&mut send_task) => recv_task.abort(), _ = (&mut recv_task) => send_task.abort(), };
}

async fn handle_vitals(State(state): State<Arc<AppState>>) -> Json<VitalsResponse> {
    let v = state.monitor.get_vitals();
    let res = atomic_to_f32(state.current_resonance.load(Ordering::Relaxed));
    Json(VitalsResponse { status: v.status, cpu_usage: v.cpu_load * 10.0, ram_usage: format!("{:.2}GB / {:.2}GB", v.ram_used_gb, v.ram_total_gb), resonance: res })
}

async fn handle_vision_frame() -> Json<NeuralResponse> {
    match vision::capture_frame().await {
        Ok(frame) => Json(NeuralResponse { response: frame, status: "ok".into() }),
        Err(e) => Json(NeuralResponse { response: format!("Capture Error: {}", e), status: "fail".into() })
    }
}

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    if payload.prompt.trim().to_lowercase() == "reindex" {
        return match KnowledgeBroker::deep_index(state.clone()).await {
            Ok(msg) => Json(NeuralResponse { response: msg, status: "indexed".into() }),
            Err(e) => Json(NeuralResponse { response: format!("Fail: {}", e), status: "fail".into() }),
        };
    }
    
    // Call Dissection Logic if requested
    if payload.prompt.to_lowercase().starts_with("dissect ") {
        let path = payload.prompt.replace("dissect ", "").trim().to_string();
        return Json(NeuralResponse { response: perform_dissection(&path).await, status: "complete".into() });
    }

    let vitals = state.monitor.get_vital_signs_string();
    let resonance = atomic_to_f32(state.current_resonance.load(Ordering::Relaxed));
    let packet = ContextPacket { 
        file_content: payload.context.clone(), 
        terminal_log: payload.terminal_context.clone(),
        memory_recall: Some(state.memory.recall()),
        system_stats: Some(vitals),
        image_b64: if payload.use_vision.unwrap_or(false) { vision::capture_frame().await.ok() } else { None },
    };
    match state.agent.prepare_and_run(&state.client, &state.api_key, &payload.prompt, packet, resonance).await {
        Ok((r, d)) => {
            if r.contains("```") {
                state.memory.remember(payload.prompt.clone(), r.clone(), payload.terminal_context.unwrap_or_default(), None);
            }
            Json(NeuralResponse { response: r, status: format!("{}ms", d) })
        },
        Err(e) => Json(NeuralResponse { response: e.to_string(), status: "fail".into() })
    }
}

async fn handle_list_files(Query(params): Query<FileParams>) -> Json<Vec<DirEntry>> {
    let path_str = if params.path.is_empty() { "." } else { &params.path };
    let mut entries = Vec::new();
    if params.recursive.unwrap_or(false) { read_dir_recursive(StdPath::new(path_str), &mut entries); } else if let Ok(rd) = fs::read_dir(path_str) { for entry in rd.flatten() { let name = entry.file_name().to_string_lossy().to_string(); if !name.starts_with('.') && name != "target" && name != "node_modules" && name != "ui" { entries.push(DirEntry { name, is_dir: entry.path().is_dir(), path: entry.path().to_string_lossy().into() }); } } }
    Json(entries)
}

async fn handle_build(Json(payload): Json<FileContent>) -> Json<polyglot::BuildResult> { Json(polyglot::detect_and_build(&payload.path)) }
async fn handle_deploy(Json(req): Json<DeployReq>) -> Json<String> { match PodmanEngine::deploy(&req.image, &req.port) { Ok(id) => Json(id), Err(e) => Json(format!("Deploy Error: {}", e)) } }
async fn handle_nuke(Json(req): Json<NukeReq>) -> Json<String> { match PodmanEngine::nuke(&req.id) { Ok(_) => Json("Nuked".into()), Err(e) => Json(format!("Error: {}", e)) } }
async fn handle_cleanup() -> Json<String> { match crate::janitor::Janitor::cleanup_build_artifacts() { Ok(m) => Json(m), Err(e) => Json(e.to_string()) } }
async fn handle_read_file(Query(params): Query<FileParams>) -> impl IntoResponse { match fs::read_to_string(&params.path) { Ok(c) => (StatusCode::OK, Json(FileContent { path: params.path, content: c })).into_response(), Err(_) => StatusCode::NOT_FOUND.into_response(), } }
async fn handle_write_file(Json(payload): Json<FileContent>) -> impl IntoResponse {
    if let Err(e) = crate::auditor::SyntaxSentinel::validate_safety(&payload.path, &payload.content) { return (StatusCode::BAD_REQUEST, Json(format!("Security Block: {}", e))).into_response(); }
    if let Some(parent) = StdPath::new(&payload.path).parent() { let _ = fs::create_dir_all(parent); }
    crate::industrial::IndustrialDrone::archive_file(&payload.path).ok(); match fs::write(&payload.path, &payload.content) {
        Ok(_) => { if payload.path.ends_with(".rs") { let _ = Command::new("rustfmt").arg(&payload.path).output(); } (StatusCode::OK, Json("Saved".to_string())).into_response() },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response()
    }
}
async fn handle_get_memories(State(state): State<Arc<AppState>>) -> Json<Vec<crate::memory::Memory>> { Json(state.memory.load_all()) }
async fn handle_purge_memories(State(state): State<Arc<AppState>>) -> StatusCode { state.memory.purge(); StatusCode::OK }
async fn handle_hive_status() -> Json<Vec<crate::podman::Container>> { Json(PodmanEngine::list()) }
async fn handle_hive_spawn(Json(req): Json<HiveReq>) -> Json<String> { match PodmanEngine::spawn_drone(&req.drone) { Ok(id) => Json(format!("Drone {} Online: {}", req.drone, id)), Err(e) => Json(format!("Spawn Failed: {}", e.to_string())) } }
async fn handle_hive_kill() -> StatusCode { let _ = PodmanEngine::kill_all(); StatusCode::OK }
async fn handle_graph(State(state): State<Arc<AppState>>) -> Json<NeuralGraph> { Json(KnowledgeBroker::get_graph(state).await) }

async fn index_handler() -> impl IntoResponse { static_handler(Uri::from_static("/index.html")).await }
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string(); if path.is_empty() { path = "index.html".to_string(); }
    match Assets::get(&path) { 
        Some(content) => { let mime = mime_guess::from_path(&path).first_or_octet_stream(); ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response() }, 
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(), 
    }
}
