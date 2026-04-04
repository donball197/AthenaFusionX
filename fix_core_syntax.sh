#!/bin/bash
echo ">> [NEXUS] INITIATING SYNTAX REPAIR SEQUENCE"

# --- 1. REPAIR memory.rs (Purging shell artifacts) ---
echo "[*] Restoring src/memory.rs..."
cat << 'EOF' > src/memory.rs
use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

const SLAB_SIZE: usize = 4096;

struct FreeNode {
    next: Option<NonNull<FreeNode>>,
}

pub struct SlabAllocator {
    block_size: usize,
    free_list: Option<NonNull<FreeNode>>,
    pages: Vec<*mut u8>,
}

unsafe impl Send for SlabAllocator {}
unsafe impl Sync for SlabAllocator {}

impl SlabAllocator {
    pub fn new(block_size: usize) -> Self {
        let block_size = block_size.max(std::mem::size_of::<FreeNode>());
        Self { block_size, free_list: None, pages: Vec::new() }
    }

    pub fn allocate(&mut self) -> Option<*mut u8> {
        if let Some(node_ptr) = this.free_list {
            unsafe {
                let node = node_ptr.as_ptr();
                this.free_list = (*node).next;
                return Some(node as *mut u8);
            }
        }
        if self.grow() { self.allocate() } else { None }
    }

    fn grow(&mut self) -> bool {
        unsafe {
            let layout = Layout::from_size_align(SLAB_SIZE, SLAB_SIZE).unwrap();
            let page = alloc(layout);
            if page.is_null() { return false; }
            self.pages.push(page);
            let num_blocks = SLAB_SIZE / self.block_size;
            for i in 0..num_blocks {
                let block_ptr = page.add(i * self.block_size);
                self.deallocate(block_ptr);
            }
            true
        }
    }

    pub fn deallocate(&mut self, ptr: *mut u8) {
        unsafe {
            let node = ptr as *mut FreeNode;
            (*node).next = self.free_list;
            self.free_list = Some(NonNull::new_unchecked(node));
        }
    }
}

impl Drop for SlabAllocator {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(SLAB_SIZE, SLAB_SIZE).unwrap();
        for page in &self.pages {
            unsafe { dealloc(*page, layout) };
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub timestamp: u64,
}

pub struct MemoryPalace {
    pub path: PathBuf,
    pub entries: Vec<MemoryEntry>,
    pub allocator: SlabAllocator,
}

impl MemoryPalace {
    pub fn init(path: PathBuf) -> Self {
        Self {
            path,
            entries: Vec::new(),
            allocator: SlabAllocator::new(128),
        }
    }
    pub fn new() -> Self { Self::init(PathBuf::from(".nexus_memory")) }
    pub fn load_all(&self) -> Result<Vec<MemoryEntry>, String> { Ok(this.entries.clone()) }
    pub fn purge_to_archive(&mut self, _summary: String) { self.entries.clear(); }
}
pub type Memory = MemoryEntry;
EOF

# --- 2. REPAIR server.rs (Fixing the broken Axum handlers) ---
echo "[*] Restoring src/server.rs..."
cat << 'EOF' > src/server.rs
use ax_core = axum;
use axum::{
    extract::{Query, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};
use tower_http::cors::CorsLayer;
use futures_util::{SinkExt, StreamExt};
use walkdir::WalkDir;
use mime_guess;
use crate::{arbiter_agent::ArbiterAgent, local_llm::LocalLLM, neural_agent::MicroAgent, scheduler::NexusTask, system_monitor::SystemMonitor, sandbox};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client: reqwest::Client,
    pub monitor: SystemMonitor,
    pub agent: Arc<MicroAgent>,
    pub orchestrator: Arc<crate::orchestrator::Orchestrator>,
    pub memory: Arc<tokio::sync::Mutex<crate::memory::MemoryPalace>>,
    pub api_key: String,
    pub current_resonance: Arc<std::sync::atomic::AtomicU32>,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub scheduler_tx: tokio::sync::mpsc::Sender<NexusTask>,
    pub arbiter: Arc<ArbiterAgent>,
    pub local_llm: Arc<LocalLLM>,
    pub workspace_root: std::path::PathBuf,
}

pub async fn start_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/fs/list", get(handle_fs_list))
        .route("/api/fs/read", get(handle_read_file))
        .route("/api/fs/write", post(handle_write_file))
        .route("/api/fs/delete", delete(handle_delete_file))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

async fn handle_fs_list(Query(p): Query<ListParams>) -> impl IntoResponse {
    let path = p.path.as_deref().unwrap_or("src");
    let mut entries = Vec::new();
    if let Ok(safe_path) = sandbox::validate_path(path) {
        for entry in WalkDir::new(safe_path).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            entries.push(FsEntry { 
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                is_dir: entry.path().is_dir()
            });
        }
        (StatusCode::OK, Json(entries)).into_response()
    } else {
        (StatusCode::FORBIDDEN, "Access Denied").into_response()
    }
}

async fn handle_read_file(Query(p): Query<FileParams>) -> impl IntoResponse {
    match sandbox::validate_path(&p.path) {
        Ok(safe) => match std::fs::read_to_string(safe) {
            Ok(c) => (StatusCode::OK, Json(serde_json::json!({ "content": c }))).into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
        },
        Err(e) => (StatusCode::FORBIDDEN, e).into_response(),
    }
}

async fn handle_write_file(Json(payload): Json<FileContent>) -> impl IntoResponse {
    match sandbox::validate_path(&payload.path) {
        Ok(safe) => {
            if let Some(p) = safe.parent() { let _ = std::fs::create_dir_all(p); }
            match std::fs::write(safe, &payload.content) {
                Ok(_) => StatusCode::OK.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        },
        Err(e) => (StatusCode::FORBIDDEN, e).into_response(),
    }
}

async fn handle_delete_file(Query(p): Query<FileParams>) -> impl IntoResponse {
    match sandbox::validate_path(&p.path) {
        Ok(safe) => match std::fs::remove_file(safe) {
            Ok(_) => StatusCode::OK.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::FORBIDDEN, e).into_response(),
    }
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };
    match Assets::get(&final_path) {
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&final_path).first_or_octet_stream().as_ref())], content.data).into_response(),
        None => if !final_path.contains('.') { ([(header::CONTENT_TYPE, "text/html")], Assets::get("index.html").unwrap().data).into_response() } else { StatusCode::NOT_FOUND.into_response() }
    }
}

#[derive(Deserialize)] pub struct FileParams { pub path: String }
#[derive(Deserialize)] pub struct ListParams { pub path: Option<String> }
#[derive(Deserialize, Serialize)] pub struct FileContent { pub path: String, pub content: String }
#[derive(Serialize)] pub struct FsEntry { pub name: String, pub path: String, pub is_dir: bool }
EOF

# --- 3. REPAIR executor.rs (Ensuring sandbox is used) ---
echo "[*] Restoring src/executor.rs..."
cat << 'EOF' > src/executor.rs
use std::fs;
use crate::nexus_error::NexusError;
use crate::agent_protocol::AgentAction;
use crate::diff::DiffEngine;
use crate::sandbox;

pub struct Executor;

impl Executor {
    pub async fn execute(actions: Vec<AgentAction>) -> Result<Vec<String>, NexusError> {
        let mut results = Vec::new();
        for action in actions {
            match action {
                AgentAction::ReadFile { path } => {
                    let safe = sandbox::validate_path(&path).map_err(|e| NexusError::SecurityViolation { message: e, cvss: 7.5 })?;
                    let c = fs::read_to_string(safe)?;
                    results.push(format!("SUCCESS: Content of {}:\n{}", path, c));
                }
                AgentAction::PatchFile { path, search, replace } => {
                    let safe = sandbox::validate_path(&path).map_err(|e| NexusError::SecurityViolation { message: e, cvss: 7.5 })?;
                    let original = fs::read_to_string(&safe)?;
                    let new_content = DiffEngine::apply(&original, &search, &replace).map_err(|e| NexusError::Other(e.to_string()))?;
                    fs::write(safe, new_content)?;
                    results.push(format!("SUCCESS: Patched {}", path));
                }
                AgentAction::WriteFile { path, content } => {
                    let safe = sandbox::validate_path(&path).map_err(|e| NexusError::SecurityViolation { message: e, cvss: 8.8 })?;
                    if let Some(p) = safe.parent() { let _ = fs::create_dir_all(p); }
                    fs::write(safe, content)?;
                    results.push(format!("SUCCESS: Written {}", path));
                }
                AgentAction::Message { message } => { results.push(format!("MSG: {}", message)); }
                _ => {}
            }
        }
        Ok(results)
    }
}
EOF

echo ">> [NEXUS] REPAIR COMPLETE. TRIGGERING CARGO CHECK."
cargo check
