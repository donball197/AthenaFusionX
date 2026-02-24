#![allow(dead_code, unused_variables)]
pub mod auto_healer;
mod server;
mod neural_agent;
mod memory;
pub mod system_monitor;
mod vision;
mod podman;
mod polyglot;
mod hive_mind;
mod auditor;
mod broker;
pub mod janitor;
mod sentry;
mod resonator;
mod industrial;
mod devops;
mod hiram_core;
mod mission_control;

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;
use crate::neural_agent::MicroAgent;
use crate::memory::MemoryPalace;
use crate::resonator::ResonatorCore;
use crate::podman::PodmanEngine;
use crate::industrial::IndustrialDrone;
use crate::devops::DevOpsDrone;
use sysinfo::System;

fn f32_to_atomic(val: f32) -> u32 { val.to_bits() }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    print!("\x1B[2J\x1B[1;1H");
    println!(">> [NEXUS: SINGULARITY] HYBRID CORE IGNITION...");
    
    let api_key = std::env::var("GEMMA_API_KEY").expect("GEMMA_API_KEY must be set");
    
    // 1. ZIG BRIDGE CHECK
    // Only attempt if model exists to avoid confusing error logs
    if std::path::Path::new("vault/gemma3n_e2b.gguf").exists() {
        match ResonatorCore::ignite_vault("vault/gemma3n_e2b.gguf") {
            Ok(msg) => println!(">> [VAULT] Zig Resonator: ONLINE ({})", msg),
            Err(_) => println!(">> [VAULT] Zig Bridge: Failed to bind."),
        }
    } else {
        println!(">> [VAULT] No GGUF found. Running Cloud-Only Mode.");
    }

    // 2. PODMAN CHECK
    let _ = PodmanEngine::ensure_synapse();

    // 3. ATOMIC RESONANCE
    let shared_resonance = Arc::new(std::sync::atomic::AtomicU32::new(f32_to_atomic(0.0)));
    let monitor_resonance = shared_resonance.clone();

    // Background Thread: Zig Entropy Scan
    tokio::spawn(async move {
        let target = "vault/gemma3n_e2b.gguf";
        loop {
            if std::path::Path::new(target).exists() {
                if let Ok(energy) = ResonatorCore::scan_local_energy(target) {
                    monitor_resonance.store(f32_to_atomic(energy), std::sync::atomic::Ordering::Relaxed);
                }
            }
            sleep(Duration::from_secs(10)).await;
        }
    });

    let sys = Arc::new(std::sync::Mutex::new(System::new_all()));
    
    let state = Arc::new(server::AppState {
        client: reqwest::Client::new(),
        brain_endpoint: "gemma-3-27b-it".to_string(),
        monitor: SystemMonitor::new(sys.clone()),
        agent: MicroAgent::init().expect("Failed to initialize MicroAgent"),
        memory: MemoryPalace::new(),
        api_key,
        hive: crate::hive_mind::HiveMind::connect().ok(),
        current_resonance: shared_resonance,
        hiram: Arc::new(tokio::sync::Mutex::new(crate::hiram_core::Hiram::new(60))),
        last_health_error: Arc::new(tokio::sync::Mutex::new(None)),
    });

    let healer_state = state.last_health_error.clone();
    let hiram_arc = state.hiram.clone();
    
    // 4. AUTONOMOUS WATCHDOGS
    tokio::spawn(async move {
        loop {
            // Pulse Checks
            let diag = crate::auto_healer::AutoHealer::run_diagnostic().await;
            {
                let mut hiram = hiram_arc.lock().await;
                hiram.verify();
            }
            // Auto-Archive & Throttle
            DevOpsDrone::run_background_check(".", "nexus-core");
            let _ = IndustrialDrone::auto_throttle().await;

            *healer_state.lock().await = diag;
            sleep(Duration::from_secs(10)).await;
        }
    });

    println!(">> [NEXUS] SYSTEMS GREEN. LISTENING ON PORT 3000.");
    server::start_server(state).await;
}
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
use crate::hiram_core::Hiram;

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
    pub hiram: Arc<tokio::sync::Mutex<Hiram>>,
    pub last_health_error: Arc<tokio::sync::Mutex<Option<String>>>,
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
#[derive(Deserialize)] pub struct ForgeReq { pub name: String, pub dockerfile: String }
#[derive(Deserialize)] pub struct PredictReq { pub content: String, pub cursor: usize, pub path: String }
#[derive(Serialize)] pub struct PredictRes { pub prediction: String }
#[derive(Deserialize)] pub struct MissionPayload { pub path: String, pub content: String }

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
        .route("/api/predict", post(handle_predict)) 
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file)) 
        .route("/api/fs/list", get(handle_list_files))
        .route("/api/mission", post(handle_mission)) 
        .route("/api/memories", get(handle_get_memories))
        .route("/api/memories/purge", delete(handle_purge_memories))
        .route("/api/hive/status", get(handle_hive_status))
        .route("/api/hive/spawn", post(handle_hive_spawn))
        .route("/api/hive/kill_all", post(handle_hive_kill))
        .route("/api/hive/forge", post(handle_hive_forge)) 
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

async fn handle_predict(State(state): State<Arc<AppState>>, Json(payload): Json<PredictReq>) -> Json<PredictRes> {
    let lines: Vec<&str> = payload.content.lines().collect();
    let start = lines.len().saturating_sub(20);
    let context = lines[start..].join("\n");
    
    let prompt = format!("COMPLETE CODE. NO MARKDOWN. JUST CODE.\nCONTEXT:\n{}", context);
    
    let packet = ContextPacket::default();
    // Fast inference (0 resonance) for ghost text
    let (text, _) = state.agent.prepare_and_run(&state.client, &state.api_key, &prompt, packet, 0.0).await.unwrap_or(("".into(), 0));
    
    let clean = text.replace("```rust", "").replace("```", "").trim().to_string();
    Json(PredictRes { prediction: clean })
}

async fn handle_prompt(State(state): State<Arc<AppState>>, Json(payload): Json<PromptRequest>) -> Json<NeuralResponse> {
    if payload.prompt.trim().to_lowercase() == "reindex" {
        return match KnowledgeBroker::deep_index(state.clone()).await {
            Ok(msg) => Json(NeuralResponse { response: msg, status: "indexed".into() }),
            Err(e) => Json(NeuralResponse { response: format!("Fail: {}", e), status: "fail".into() }),
        };
    }
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
            // LAYER 13 INTERCEPT: Check for Drone Forge Requests
            if r.contains("DOCKERFILE:") {
                println!(">> [ARCHITECT] Blueprint Detected.");
            }
            if r.contains("```") {
                state.memory.remember(payload.prompt.clone(), r.clone(), payload.terminal_context.unwrap_or_default(), None);
            }
            Json(NeuralResponse { response: r, status: format!("{}ms", d) })
        },
        Err(e) => Json(NeuralResponse { response: e.to_string(), status: "fail".into() })
    }
}

// ... (Rest of handlers: dissect, ws, vitals, vision, files, build, etc.)
// Include them as they were in previous version, they are stable.
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
    match vision::capture_frame().await { Ok(frame) => Json(NeuralResponse { response: frame, status: "ok".into() }), Err(e) => Json(NeuralResponse { response: format!("Capture Error: {}", e), status: "fail".into() }) }
}
async fn handle_mission(State(state): State<Arc<AppState>>, Json(payload): Json<MissionPayload>) -> Json<serde_json::Value> {
    let _ = crate::industrial::IndustrialDrone::archive_file(&payload.path);
    match fs::write(&payload.path, &payload.content) { Ok(_) => Json(serde_json::json!({"status": "Success"})), Err(e) => Json(serde_json::json!({"status": "Error", "message": e.to_string()})) }
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
async fn handle_write_file(State(state): State<Arc<AppState>>, Json(payload): Json<FileContent>) -> impl IntoResponse {
    if let Err(e) = crate::auditor::SyntaxSentinel::validate_safety(&payload.path, &payload.content) { return (StatusCode::BAD_REQUEST, Json(format!("Security Block: {}", e))).into_response(); }
    if let Some(parent) = StdPath::new(&payload.path).parent() { let _ = fs::create_dir_all(parent); }
    let _ = crate::industrial::IndustrialDrone::archive_file(&payload.path);
    match fs::write(&payload.path, &payload.content) {
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
async fn handle_hive_forge(Json(req): Json<ForgeReq>) -> Json<String> { match PodmanEngine::forge_drone(&req.name, &req.dockerfile) { Ok(msg) => Json(msg), Err(e) => Json(format!("Forge Error: {}", e)) } }
async fn index_handler() -> impl IntoResponse { static_handler(Uri::from_static("/index.html")).await }
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string(); if path.is_empty() { path = "index.html".to_string(); }
    match Assets::get(&path) { Some(content) => { let mime = mime_guess::from_path(&path).first_or_octet_stream(); ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response() }, None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(), }
}
use anyhow::Result;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Instant;
use std::sync::Mutex;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct ContextPacket {
    pub file_content: Option<String>,
    pub terminal_log: Option<String>, 
    pub memory_recall: Option<String>,
    pub system_stats: Option<String>, 
    pub image_b64: Option<String>, 
}

pub struct MicroAgent { memory: Mutex<VecDeque<String>> }

impl MicroAgent {
    pub fn init() -> Result<Self> { 
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(8)) }) 
    }

    pub async fn prepare_and_run(&self, client: &Client, api_key: &str, user_text: &str, context: ContextPacket, resonance: f32) -> Result<(String, u128)> {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);
        
        let temperature = 0.1 + (resonance * 0.8);
        let mood = if resonance > 0.7 { "ARCHITECT" } else { "ENGINEER" };
        
        // ALPINE SYSTEM PROMPT (Optimized for Layer 13)
        let system_instruction = format!(
            "You are NEXUS (Alpine Edition). Resonance: {:.2} (Mode: {}).
             PROTOCOL:
             1. CODE: Rust/Axum. Output full files with 'FILE: path'.
             2. HIVE: To create drones, use:
                DOCKERFILE: name
                ```dockerfile
                FROM alpine:latest
                RUN apk add ...
                ```
             3. CONSTRAINT: Prefer Alpine Linux. Keep it small.",
            resonance, mood
        );

        let mut history_string = String::new();
        { 
            let history = self.memory.lock().unwrap(); 
            for msg in history.iter() { history_string.push_str(msg); history_string.push_str("\n"); } 
        }

        let mut context_block = String::new();
        if let Some(stats) = context.system_stats { context_block.push_str(&format!("\n[VITALS]\n{}\n", stats)); }
        if let Some(code) = context.file_content { context_block.push_str(&format!("\n[ACTIVE]\n{}\n", code)); }
        if let Some(log) = context.terminal_log { context_block.push_str(&format!("\n[LOGS]\n{}\n", log)); }

        let text_part = json!({ 
            "text": format!("{}\n\nCONTEXT:\n{}\n\nHISTORY:\n{}\n\nUSER: {}", system_instruction, context_block, history_string, user_text) 
        });
        
        let mut parts = vec![text_part];
        if let Some(img_data) = context.image_b64 {
            let clean_b64 = if let Some(idx) = img_data.find(',') { img_data[idx+1..].to_string() } else { img_data };
            parts.push(json!({ "inline_data": { "mime_type": "image/jpeg", "data": clean_b64 } }));
        }

        let payload = json!({ 
            "contents": [{ "parts": parts }], 
            "generationConfig": { "temperature": temperature, "maxOutputTokens": 8192 } 
        });
        
        let start = Instant::now();
        let resp = client.post(url).json(&payload).send().await?;
        let duration = start.elapsed().as_millis();
        
        let body: Value = resp.json().await?;
        let text = body["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("...").to_string();

        { 
            let mut history = self.memory.lock().unwrap(); 
            history.push_back(format!("U: {}", user_text)); 
            history.push_back(format!("N: {}", text)); 
            while history.len() > 8 { history.pop_front(); } 
        }
        Ok((text, duration))
    }
}
use std::process::Command;
use serde::Serialize;
use anyhow::Result;
use std::fs;

#[derive(Serialize, Debug, Clone)]
pub struct Container { pub id: String, pub image: String, pub status: String, pub names: String }

pub struct PodmanEngine;

impl PodmanEngine {
    pub fn ensure_synapse() -> Result<()> {
        let check = Command::new("podman").args(&["ps", "-q", "-f", "name=nexus_synapse"]).output()?;
        if check.stdout.is_empty() {
            Command::new("podman").args(&["run", "-d", "--name", "nexus_synapse", "-p", "6379:6379", "--restart", "always", "redis:alpine"]).output()?;
        }
        Ok(())
    }

    pub fn list() -> Vec<Container> {
        let output = Command::new("podman").args(&["ps", "-a", "--format", "{{.ID}}|{{.Image}}|{{.Status}}|{{.Names}}"]).output();
        if let Ok(out) = output {
            let raw = String::from_utf8_lossy(&out.stdout);
            raw.lines().filter_map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    Some(Container { id: parts[0].to_string(), image: parts[1].to_string(), status: parts[2].to_string(), names: parts[3].to_string() })
                } else { None }
            }).collect()
        } else { Vec::new() }
    }

    pub fn spawn_drone(drone_type: &str) -> Result<String> {
        // ALPINE SWITCHOVER: We replaced Kali with Alpine SecOps
        // FIX: The second element (name) must be an owned String to satisfy the borrow checker (E0716).
        let (image, name, ports) = match drone_type {
            "secops" => ("nexus_secops_alpine", "nexus_secops".to_string(), None), // Light
            "devops" => ("node:18-alpine", "nexus_devops".to_string(), None),      // Light
            "industrial" => ("postgres:alpine", "nexus_industrial".to_string(), Some("5432")),
            // 13th Layer Fallback (Architect Protocol)
            custom => (custom, format!("nexus_{}", custom), None),
        };
        
        let _ = Command::new("podman").args(&["rm", "-f", &name]).output();
        
        let mut cmd = Command::new("podman");
        cmd.arg("run").arg("-d").arg("--name").arg(&name).arg("--network").arg("slirp4netns:allow_host_loopback=true");
        if let Some(p) = ports { cmd.arg("-p").arg(format!("{}:{}", p, p)); }
        cmd.arg(image).arg("sleep").arg("infinity");
        
        let res = cmd.output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }

    pub fn forge_drone(name: &str, dockerfile: &str) -> Result<String> {
        let build_dir = format!("hives/{}", name);
        fs::create_dir_all(&build_dir)?;
        fs::write(format!("{}/Dockerfile", build_dir), dockerfile)?;
        
        // Force Alpine base if the AI forgets
        if !dockerfile.contains("FROM alpine") {
            println!(">> [ARCHITECT] Warning: Non-Alpine base detected. Proceeding with caution.");
        }

        let output = Command::new("podman").args(&["build", "-t", name, &build_dir]).output()?;
        if output.status.success() { Ok(format!("Forged {}", name)) } else { Err(anyhow::anyhow!("Forge Failed")) }
    }

    pub fn kill_all() -> Result<()> {
        for c in Self::list() { if c.names != "nexus_synapse" { let _ = Command::new("podman").args(&["stop", &c.id]).output(); } }
        Ok(())
    }

    pub fn deploy(image: &str, port: &str) -> Result<String> {
        let res = Command::new("podman").args(&["run", "-d", "-p", &format!("{}:{}", port, port), image]).output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }

    pub fn nuke(id: &str) -> Result<()> { let _ = Command::new("podman").args(&["rm", "-f", id]).output(); Ok(()) }
}
<script>
    import { onMount, tick } from 'svelte';
    import Editor from '$lib/components/Editor.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import Vitals from '$lib/components/Vitals.svelte';
    import Heartbeat from '$lib/components/Heartbeat.svelte'; 
    import TacticalBlade from '$lib/components/TacticalBlade.svelte';
    import Voice from '$lib/components/Voice.svelte';
    import ResponseModule from '$lib/components/ResponseModule.svelte';
    import GhostCortex from "$lib/components/GhostCortex.svelte";
    import MergeGate from '$lib/components/MergeGate.svelte'; 
    import SwarmMap from '$lib/components/SwarmMap.svelte';
    import SoundFx from '$lib/SoundFx.js'; 

    let currentFile = "src/main.rs";
    let code = "// Hive Ready.";
    let activeTab = 'editor'; 
    let userPrompt = "";
    let neuralResponse = ">> HIVE ONLINE";
    let isThinking = false;
    let terminalInstance;
    let bladeOpen = false; 
    let drones = []; 
    let patchAvailable = false;
    let patchContent = "";
    let targetPatchFile = ""; 
    let visionActive = false;
    let voiceMuted = false; 
    let vimMode = false;
    let vitals = { status: "OFFLINE", cpu_usage: 0, ram_usage: "0.0", resonance: 0.0 }; 
    let ghostLogs = ""; 
    let buildOutput = "";

    function toggleMute() { voiceMuted = !voiceMuted; SoundFx.click(); }
    function toggleVim() { vimMode = !vimMode; SoundFx.success(); }

    async function openFile(path) { 
        currentFile = path; 
        const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`); 
        if (res.ok) { const data = await res.json(); code = data.content; activeTab = 'editor'; } 
    }
    
    async function spawnDrone(type) { await fetch('/api/hive/spawn', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({drone: type, action: "spawn"}) }); refreshHive(); }
    async function killAll() { await fetch('/api/hive/kill_all', { method: 'POST' }); refreshHive(); }
    async function refreshHive() { const res = await fetch('/api/hive/status'); if(res.ok) drones = await res.json(); }
    async function save() { await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: currentFile, content: code}) }); }
    
    async function applyPatch() {
        await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: targetPatchFile, content: patchContent}) });
        patchAvailable = false; SoundFx.success();
        if (targetPatchFile === currentFile) openFile(currentFile);
    }

    async function triggerBuild() {
        await save();
        activeTab = 'hive';
        buildOutput = ">> COMPILING...";
        const res = await fetch('/api/devops/build', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ path: ".", content: "" }) });
        const result = await res.json();
        buildOutput = result.log;
        
        if (!result.success) {
            SoundFx.error();
            userPrompt = `Build failed. Fix error in ${currentFile}:\n${buildOutput}`;
            executeAction();
        } else {
            SoundFx.success();
        }
    }

    async function executeAction() {
        if (!userPrompt.trim() || isThinking) return;
        isThinking = true; SoundFx.processing();
        const p = userPrompt; userPrompt = "";
        try {
            const res = await fetch('/api/prompt', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ prompt: p, context: code, use_vision: visionActive }) });
            const data = await res.json(); neuralResponse = data.response;
            
            // --- LAYER 13: THE ARCHITECT INTERCEPT ---
            if (neuralResponse.includes("DOCKERFILE:")) {
                const dockerMatch = neuralResponse.match(/DOCKERFILE:\s*([a-z0-9_]+)\s*\n```(?:dockerfile)?\s*([\s\S]*?)```/i);
                if (dockerMatch) {
                    const droneName = dockerMatch[1];
                    const dockerBody = dockerMatch[2];
                    
                    ghostLogs += `\n>> [ARCHITECT] Intercepted forge request: ${droneName}...`;
                    
                    // Call the Forge
                    const forgeRes = await fetch('/api/hive/forge', {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({name: droneName, dockerfile: dockerBody})
                    });
                    
                    if (forgeRes.ok) {
                        ghostLogs += `\n>> [ARCHITECT] Successfully forged ${droneName}. Spawning...`;
                        SoundFx.success();
                        await spawnDrone(droneName);
                    } else {
                        ghostLogs += `\n>> [ERROR] Forge failed.`;
                        SoundFx.error();
                    }
                }
            }

            // --- UNIVERSAL FABRICATOR (Multi-File) ---
            const multiFileRegex = />>> FILE: (.*?)\n([\s\S]*?)<<</g;
            let match;
            let foundFiles = false;
            while ((match = multiFileRegex.exec(neuralResponse)) !== null) {
                foundFiles = true;
                const fPath = match[1].trim();
                const fContent = match[2].trim();
                await fetch('/api/fs/write', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({path: fPath, content: fContent})
                });
                ghostLogs += `\n>> [FABRICATOR] Created ${fPath}`;
            }

            // Fallback to legacy single file patch logic
            if (!foundFiles && neuralResponse.includes("```") && !neuralResponse.includes("DOCKERFILE:")) {
                const match = neuralResponse.match(/```(?:rust|js|json)?\s*([\s\S]*?)\`\`\`/i);
                if (match) { patchContent = match[1].trim(); patchAvailable = true; }
                const fileMatch = neuralResponse.match(/FILE:\s*([^\n]+)/);
                targetPatchFile = fileMatch ? fileMatch[1].split(" ")[0].trim() : currentFile;
            }
        } catch (e) { neuralResponse = ">> ERROR: " + e.message; } finally { isThinking = false; }
    }

    onMount(() => { 
        openFile(currentFile); 
        const interval = setInterval(() => { 
            fetch('/api/vitals').then(r => r.ok ? r.json() : null).then(v => { if(v) vitals = v; }); 
            refreshHive();
        }, 3000); 
        return () => clearInterval(interval);
    });
</script>

<div class="flex flex-col h-screen w-screen bg-black text-green-500 font-mono overflow-hidden">
    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between z-50">
        <div class="flex items-center gap-4">
            <span class="text-[#58a6ff] font-bold text-[10px] tracking-widest uppercase">Nexus // Hive</span>
            <button on:click={toggleMute} class="text-xs">{voiceMuted ? '🔇' : '🔊'}</button>
        </div>
        
        <div class="flex gap-2 ml-4">
            <button on:click={() => activeTab = 'editor'} class="text-[9px] px-3 py-1 border {activeTab === 'editor' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">CODE</button>
            <button on:click={() => activeTab = 'hive'} class="text-[9px] px-3 py-1 border {activeTab === 'hive' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">HIVE</button>
            <button on:click={() => activeTab = 'terminal'} class="text-[9px] px-3 py-1 border {activeTab === 'terminal' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">TERM</button>
        </div>

        <div class="flex-1 max-w-xl px-4">
            <input 
                class="w-full bg-[#0d1117] border border-[#30363d] text-white px-3 py-1 text-[10px] outline-none focus:border-blue-500" 
                placeholder="Ask Nexus..." 
                bind:value={userPrompt} 
                on:keydown={(e) => { 
                    if (e.key === 'Enter') { 
                        e.preventDefault(); 
                        e.stopPropagation(); 
                        executeAction(); 
                    } 
                }} 
            />
        </div>

        <div class="flex gap-2">
            <button class="bg-blue-600 text-white font-bold px-3 py-1 text-[9px] rounded hover:bg-blue-500" on:click={triggerBuild}>BUILD</button>
            <button class="bg-green-600 text-black font-bold px-3 py-1 text-[9px] rounded hover:bg-green-500" on:click={save}>SAVE</button>
        </div>
    </header>

    <div class="flex-1 flex relative overflow-hidden">
        <aside class="w-64 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col hidden md:flex shrink-0">
            <Vitals ram={vitals.ram_usage} status={vitals.status} cpu={vitals.cpu_usage} resonance={vitals.resonance} />
            <Heartbeat resonance={vitals.resonance} />
            <div class="mt-4 flex-1 overflow-y-auto custom-scrollbar">
                <ResponseModule response={neuralResponse} {isThinking} {patchAvailable} onApplyPatch={applyPatch} />
            </div>
        </aside>

        <main class="flex-1 flex flex-col bg-black relative">
            {#if activeTab === 'editor'}
                <div class="absolute inset-0 flex flex-col">
                    <Editor bind:value={code} path={currentFile} {vimMode} onChange={(v) => code = v} />
                    <TacticalBlade bind:open={bladeOpen} onFileSelect={openFile} onToggleVim={toggleVim} />
                    <GhostCortex logs={ghostLogs} active={true} isThinking={isThinking} response={neuralResponse} onCommand={(cmd) => {userPrompt = cmd; setTimeout(executeAction, 50);}} onToggleFiles={() => bladeOpen = !bladeOpen} />
                </div>
            {/if}

            {#if activeTab === 'hive'}
                <div class="absolute inset-0 w-full h-full p-6 bg-[#0d1117] flex flex-col gap-6 overflow-y-auto custom-scrollbar">
                    <SwarmMap resonance={vitals.resonance} on:interrogate={(e) => { userPrompt = "Analyze " + e.detail.path; executeAction(); }} />
                    <div class="grid grid-cols-3 gap-4">
                        {#each ['secops', 'devops', 'industrial'] as type}
                            <div class="p-4 border border-[#30363d] bg-black/40 rounded">
                                <p class="text-[10px] text-gray-500 uppercase mb-2">{type} unit</p>
                                <button on:click={() => spawnDrone(type)} class="w-full py-1 border border-green-900 text-green-500 text-[9px] hover:bg-green-500 hover:text-black">SPAWN</button>
                            </div>
                        {/each}
                    </div>
                    <div class="mt-4 bg-black border border-gray-800 p-2 text-[10px] font-mono text-gray-400 h-64 overflow-y-auto">
                        <div class="border-b border-gray-800 pb-1 mb-2">BUILD OUTPUT</div>
                        <pre>{buildOutput}</pre>
                    </div>
                </div>
            {/if}
            
            {#if activeTab === 'terminal'}
                <div class="absolute inset-0 w-full h-full bg-black">
                    <Terminal bind:this={terminalInstance} />
                </div>
            {/if}
        </main>
    </div>
    <MergeGate active={patchAvailable} path={targetPatchFile} on:apply={applyPatch} on:reject={() => patchAvailable = false} />
</div>
<script>
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    import { vim } from '@replit/codemirror-vim';
    import { keymap } from "@codemirror/view";
    import { Decoration } from "@codemirror/view";
    import { StateField, StateEffect } from "@codemirror/state";

    export let value = "";
    export let path = "NO_FILE_LOADED";
    export let onChange = (val) => {};
    export let vimMode = false;

    let editorElement;
    let view;
    let ghostText = "";
    let debounceTimer;

    const addGhost = StateEffect.define();
    const ghostField = StateField.define({
        create() { return Decoration.none; },
        update(ghosts, tr) {
            ghosts = ghosts.map(tr.changes);
            for (let e of tr.effects) if (e.is(addGhost)) ghosts = e.value;
            return ghosts;
        },
        provide: f => EditorView.decorations.from(f)
    });

    const ghostTheme = EditorView.baseTheme({
        ".cm-ghost": { color: "#6b7280", fontStyle: "italic", opacity: "0.7" }
    });

    async function fetchGhostPrediction(currentContent, cursorIdx) {
        try {
            const res = await fetch('/api/predict', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ content: currentContent, cursor: cursorIdx, path: path })
            });
            if (res.ok) {
                const data = await res.json();
                if (data.prediction) {
                    ghostText = data.prediction;
                    const pos = view.state.selection.main.head;
                    const deco = Decoration.set([
                        Decoration.widget({
                            widget: new class {
                                toDOM() { 
                                    let span = document.createElement("span");
                                    span.className = "cm-ghost";
                                    span.innerText = ghostText;
                                    return span;
                                }
                            },
                            side: 1
                        }).range(pos)
                    ]);
                    view.dispatch({ effects: addGhost.of(deco) });
                }
            }
        } catch(e) {}
    }

    function mountEditor() {
        const extensions = [
            basicSetup, rust(), oneDark, ghostField, ghostTheme,
            EditorView.updateListener.of((u) => {
                if (u.docChanged) {
                    const val = u.state.doc.toString();
                    onChange(val);
                    view.dispatch({ effects: addGhost.of(Decoration.none) });
                    clearTimeout(debounceTimer);
                    debounceTimer = setTimeout(() => {
                        fetchGhostPrediction(val, view.state.selection.main.head);
                    }, 2000); 
                }
            }),
            keymap.of([{
                key: "Tab",
                run: (view) => {
                    if (ghostText) {
                        const pos = view.state.selection.main.head;
                        view.dispatch({
                            changes: {from: pos, insert: ghostText},
                            selection: {anchor: pos + ghostText.length},
                            effects: addGhost.of(Decoration.none)
                        });
                        ghostText = "";
                        return true;
                    }
                    return false;
                }
            }]),
            EditorView.theme({
                "&": { height: "100%" },
                ".cm-scroller": { overflow: "auto", paddingBottom: "180px" }
            })
        ];
        if (vimMode) extensions.push(vim());
        view = new EditorView({
            state: EditorState.create({ doc: value, extensions }),
            parent: editorElement
        });
    }

    $: if (editorElement && (vimMode === true || vimMode === false)) {
        if (view) view.destroy();
        mountEditor();
    }
    $: if (view && value !== view.state.doc.toString()) {
        const cursor = view.state.selection.main.head;
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value }, selection: { anchor: cursor } });
    }
    onDestroy(() => { if (view) view.destroy(); });
</script>

<div class="editor-container h-full w-full bg-black flex flex-col overflow-hidden">
    <div class="w-full bg-[#0a0a0a] border-b border-blue-900/40 px-4 py-1.5 grid grid-cols-[1fr_auto] items-center z-[60] gap-4">
        <div class="flex items-center space-x-3 min-w-0">
            <div class="w-1.5 h-1.5 rounded-full {path === 'NO_FILE_LOADED' ? 'bg-red-500' : 'bg-blue-500 animate-pulse'} shrink-0"></div>
            <span class="text-[10px] text-white truncate font-mono">{path}</span>
        </div>
        <div class="flex items-center space-x-4 shrink-0">
            <div class="text-[7px] text-blue-900 uppercase tracking-tighter">GHOST: {ghostText ? 'READY (TAB)' : 'IDLE'}</div>
        </div>
    </div>
    <div class="flex-grow overflow-hidden relative" bind:this={editorElement}></div>
</div>

<style>
    .editor-container { height: 100%; }
    :global(.cm-editor) { height: 100%; outline: none !important; background-color: #000 !important; }
</style>
<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();
    
    export let open = false;
    export let onFileSelect;
    export let onToggleVim; 

    let currentDirFiles = [];
    let globalIndex = [];
    let searchTerm = "";
    let selectedIndex = 0;
    
    // FORGE LOGIC
    async function createFile() {
        if (!searchTerm.trim()) return;
        // Auto-detect if user typed "folder/file.rs"
        const path = searchTerm.trim();
        await fetch('/api/fs/write', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({path: path, content: "// New File\n"})
        });
        onFileSelect(path);
        open = false;
    }

    const systemCommands = [
        { name: 'Reload System', action: () => location.reload(), icon: '🔄', color: 'text-green-400' },
        { name: 'Toggle Vim Mode', action: () => { if(onToggleVim) onToggleVim(); }, icon: '🎮', color: 'text-yellow-400' },
        { name: 'Create File: ' + (searchTerm || '...'), action: createFile, icon: '+', color: 'text-white bg-green-600 px-1 rounded' }
    ];

    // ... (Keep existing fetch logic) ...
    function getFileMeta(name, is_dir) {
        if (is_dir) return { icon: '📂', color: 'text-blue-400' };
        if (name.endsWith('.rs')) return { icon: '🦀', color: 'text-orange-400' };
        return { icon: '📄', color: 'text-gray-500' };
    }

    async function buildGlobalIndex() {
        try {
            const res = await fetch(`/api/fs/list?path=.&recursive=true`);
            if (res.ok) { globalIndex = await res.json(); fetchDirectory("."); }
        } catch (e) {}
    }

    async function fetchDirectory(path) {
        if (globalIndex.length > 0) {
            currentDirFiles = globalIndex.filter(f => f.path.startsWith(path === "." ? "" : path));
        }
    }

    $: if (open && globalIndex.length === 0) buildGlobalIndex();

    $: displayList = (() => {
        const term = searchTerm.trim();
        if (term.startsWith('>')) return systemCommands.filter(c => !c.name.startsWith('Create'));
        // If typing a filename that doesn't exist, show "Create" option at top
        const exactMatch = globalIndex.find(f => f.path === term);
        let list = globalIndex.filter(f => f.path.includes(term)).slice(0, 50);
        
        if (term.length > 0 && !exactMatch) {
            return [systemCommands[2], ...list]; // Add Create option
        }
        return list;
    })();

    async function handleSelect(item) {
        if (item.action) { await item.action(); open = false; return; }
        onFileSelect(item.path); open = false;
    }

    // ... (Keep existing key handlers) ...
    function handleKeyDown(e) {
        if (!open) return;
        const max = displayList.length;
        if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = (selectedIndex + 1) % max; }
        else if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = (selectedIndex - 1 + max) % max; }
        else if (e.key === 'Enter') { e.preventDefault(); if (displayList[selectedIndex]) handleSelect(displayList[selectedIndex]); }
    }
</script>

<svelte:window on:keydown={(e) => { if ((e.ctrlKey || e.metaKey) && e.key === 'k') { e.preventDefault(); open = !open; searchTerm=""; } if(e.key==='Escape') open=false; }} />

{#if open}
    <div class="fixed inset-0 bg-black/50 backdrop-blur-[2px] z-[999]" on:click={() => open = false}></div>
    <div class="absolute bg-[#050505] border border-blue-900/50 rounded-sm z-[1000] shadow-2xl flex flex-col font-mono" style="left: 400px; top: 80px; width: 320px; max-height: 480px;">
        <div class="p-2 bg-[#080808]">
            <input type="text" bind:value={searchTerm} on:keydown={handleKeyDown} placeholder="> CMD or File..." class="w-full bg-[#111] border border-blue-900/30 text-white px-2 py-1.5 rounded outline-none focus:border-blue-500" autoFocus />
        </div>
        <div class="overflow-y-auto p-1 max-h-[300px] custom-scrollbar">
            {#each displayList as item, i}
                {@const meta = item.action ? {icon: item.icon, color: item.color} : getFileMeta(item.name, item.is_dir)}
                <button on:click={() => handleSelect(item)} class="w-full text-left px-2 py-1.5 text-[10px] flex items-center gap-2 {i === selectedIndex ? 'bg-blue-900/30 text-white' : 'hover:bg-[#111] text-gray-400'}">
                    <span class="{meta.color}">{item.is_back ? '⤴' : meta.icon}</span>
                    <span class="truncate">{item.name}</span>
                </button>
            {/each}
        </div>
    </div>
{/if}
<script>
    import Response from './Response.svelte';
    export let response = "";
    export let isThinking = false;
    export let patchAvailable = false;
    export let onApplyPatch;
</script>

<div class="flex flex-col min-h-[200px] max-h-[400px] bg-[#050505] border border-blue-900/30 p-0 rounded shadow-2xl relative overflow-hidden group">
    <!-- Header -->
    <div class="bg-[#0a0a0a] border-b border-blue-900/20 px-3 py-1 flex justify-between items-center">
        <span class="text-[9px] font-bold text-blue-500 uppercase tracking-widest">Neural Link</span>
        {#if isThinking}
            <div class="w-2 h-2 rounded-full bg-blue-500 animate-ping"></div>
        {/if}
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-3">
        <Response {response} {isThinking} patchAvailable={false} />
    </div>

    <!-- ACTION GATE -->
    {#if patchAvailable}
        <div class="absolute bottom-0 left-0 right-0 p-2 bg-gradient-to-t from-black to-transparent">
            <button 
                on:click={onApplyPatch}
                class="w-full bg-green-600 hover:bg-green-500 text-black font-bold py-2 text-xs rounded uppercase tracking-widest shadow-[0_0_15px_rgba(34,197,94,0.4)] animate-pulse"
            >
                [ APPLY PATCH ]
            </button>
        </div>
    {/if}
</div>
<script>
    import { onMount, onDestroy } from 'svelte';
    
    let logs = [];
    let socket;
    let inputCmd = "";
    let terminalDiv;

    // External API for parent components
    export function getLogs() {
        return logs.join("\n");
    }

    export function triggerCommand(cmd) {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(cmd + "\n");
        }
    }

    onMount(() => {
        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);

        socket.onmessage = (event) => {
            // Simple ANSI strip regex for cleaner output on low-end device
            const clean = event.data.replace(/\u001b\[[0-9;]*m/g, "");
            logs = [...logs, clean].slice(-100); // Keep last 100 lines
            if (terminalDiv) terminalDiv.scrollTop = terminalDiv.scrollHeight;
        };

        socket.onclose = () => { logs = [...logs, ">> CONNECTION LOST. RECONNECTING..."]; };
    });

    function handleKey(e) {
        if (e.key === 'Enter') {
            triggerCommand(inputCmd);
            inputCmd = "";
        }
    }
</script>

<div class="h-full w-full bg-black text-green-500 font-mono text-xs p-2 flex flex-col" style="font-family: 'JetBrains Mono', monospace;">
    <div class="flex-1 overflow-y-auto whitespace-pre-wrap" bind:this={terminalDiv}>
        {#each logs as log}
            <div>{log}</div>
        {/each}
    </div>
    <div class="flex mt-2 border-t border-gray-800 pt-1">
        <span class="mr-2">$</span>
        <input 
            class="flex-1 bg-transparent outline-none text-white" 
            bind:value={inputCmd} 
            on:keydown={handleKey} 
            autoFocus
        />
    </div>
</div>
