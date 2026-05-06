use nexus_substrate::token_ledger;
use nexus_substrate::memory::MemoryPalace;
use nexus_substrate::vmo_handler;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router, middleware,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tokio::net::TcpListener;

use crate::{
    auth, cluster_api, logic_engine::LogicEngine, security,
    vitals_handler, ws_bridge,
};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client:            reqwest::Client,
    pub monitor:           Arc<tokio::sync::Mutex<crate::system_monitor::SystemMonitor>>,
    pub agent:             Arc<crate::neural_agent::MicroAgent>,
    pub orchestrator:      Arc<crate::orchestrator::Orchestrator>,
    pub memory:            Arc<tokio::sync::Mutex<MemoryPalace>>,
    pub api_key:           String,
    pub current_resonance: Arc<std::sync::atomic::AtomicU32>,
    pub tx:                tokio::sync::broadcast::Sender<String>,
    pub scheduler_tx:      tokio::sync::mpsc::Sender<crate::scheduler::NexusTask>,
    pub arbiter:           Arc<crate::arbiter_agent::ArbiterAgent>,
    pub workspace_root:    PathBuf,
    pub vmo_state:         Arc<tokio::sync::Mutex<crate::server::VmoState>>,
    pub cluster:           Arc<crate::cluster::ClusterMesh>,
    pub dist:              Arc<crate::distributed_scheduler::DistributedScheduler>,
    pub governance_log:    Arc<parking_lot::Mutex<crate::governance::protocol::GovernanceLog>>,
}

#[derive(Default, Debug)]
pub struct VmoState {
    pub last_parent: Option<u32>,
    pub last_child:  Option<u32>,
}

#[derive(serde::Deserialize)]
pub struct Intent {
    pub intent_type: Option<String>,
    pub directive:   Option<String>,
    pub payload:     Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct LoginRequest { pub key: String }

#[derive(Deserialize)]
struct FsQuery { path: Option<String> }

fn safe_path(workspace_root: &PathBuf, user_path: &str) -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
    if user_path.is_empty() { 
        return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "PATH_EMPTY" })))); 
    }
    
    let path = std::path::Path::new(user_path);
    if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err((StatusCode::FORBIDDEN, Json(json!({ "error": "PATH_TRAVERSAL_DENIED" }))));
    }
    
    Ok(workspace_root.join(path))
}

pub async fn login_handler(Json(payload): Json<LoginRequest>) -> impl IntoResponse {
    let server_key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    if !server_key.is_empty() && payload.key == server_key {
        match auth::issue_sovereign_token() {
            Ok(token) => (StatusCode::OK, Json(json!({ "token": token }))).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed").into_response(),
        }
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid DNA Signature").into_response()
    }
}

async fn bootstrap_handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    if !addr.ip().is_loopback() { return (StatusCode::FORBIDDEN, "Loopback only").into_response(); }
    match std::fs::read_to_string(".nexus_ui_token") {
        Ok(token) => (StatusCode::OK, Json(json!({ "token": token.trim() }))).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "Token not yet generated").into_response(),
    }
}

pub async fn unified_gateway(ConnectInfo(addr): ConnectInfo<SocketAddr>, State(state): State<Arc<AppState>>, Json(intent): Json<Intent>) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if let Err(retry_ms) = token_ledger::http_check(&ip) {
        return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, format!("{}", retry_ms / 1000 + 1))], Json(json!({ "status": "RATE_LIMITED", "retry_after_ms": retry_ms }))).into_response();
    }
    let directive_str = intent.directive.or_else(|| intent.payload.as_ref()?.get("directive")?.as_str().map(|s| s.to_string())).unwrap_or_default();
    if !LogicEngine::is_action_safe(&directive_str) { return (StatusCode::FORBIDDEN, Json(json!({ "status": "LOGIC_VIOLATION" }))).into_response(); }
    let s = state.clone();
    let d_clone = directive_str.clone();
    tokio::spawn(async move {
        let d = crate::cli::inject_workspace_dna(&d_clone).await;
        crate::task_manager::send_task(s.orchestrator.clone(), &s.client, &s.api_key, &d, s.memory.clone(), s.vmo_state.clone(), s.tx.clone(), s.dist.clone()).await;
    });
    (StatusCode::OK, Json(json!({ "status": "QUEUED" }))).into_response()
}

async fn fs_read_handler(State(state): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> impl IntoResponse {
    let user_path = q.path.unwrap_or_default();
    match safe_path(&state.workspace_root, &user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => match tokio::fs::read_to_string(&safe).await {
            Ok(content) => Json(json!({ "content": content })).into_response(),
            Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
        },
    }
}

async fn fs_write_handler(State(state): State<Arc<AppState>>, Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let user_path = payload["path"].as_str().unwrap_or_default();
    let content   = payload["content"].as_str().unwrap_or_default();
    match safe_path(&state.workspace_root, user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => {
            if let Some(parent) = safe.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await { return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(); }
            }
            match tokio::fs::write(&safe, content).await {
                Ok(_) => Json(json!({ "status": "success" })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
            }
        }
    }
}

async fn fs_list_handler(State(state): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> impl IntoResponse {
    let user_path = q.path.unwrap_or_else(|| ".".into());
    match safe_path(&state.workspace_root, &user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => {
            let mut entries = Vec::new();
            if let Ok(mut read_dir) = tokio::fs::read_dir(&safe).await {
                while let Ok(Some(entry)) = read_dir.next_entry().await {
                    let meta = entry.metadata().await.ok();
                    entries.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "path": entry.path().strip_prefix(&state.workspace_root).unwrap_or(&entry.path()).to_string_lossy(),
                        "is_dir": meta.map(|m| m.is_dir()).unwrap_or(false)
                    }));
                }
            }
            Json(entries).into_response()
        }
    }
}

async fn metrics_handler() -> impl IntoResponse {
    let metrics_text = crate::metrics::get_metrics_text();
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], metrics_text)
}

pub async fn start_server(state: Arc<AppState>) {
    let api_routes = Router::new()
        .route("/task",   post(unified_gateway))
        .route("/intent", post(unified_gateway))
        .route("/vmos",   get(vmo_handler::handle_get_vmos))
        .route("/vitals", get(vitals_handler::handle_vitals))
        .route("/metrics", get(metrics_handler))
        .route("/cluster/nodes", get(cluster_api::list_nodes))
        .route("/cluster/forge", post(crate::edge_worker::run_remote_synthesis))
        .route("/cluster/run",   post(crate::edge_worker::run_remote_job))
        .route("/fs/read",  get(fs_read_handler))
        .route("/fs/write", post(fs_write_handler))
        .route("/fs/list",  get(fs_list_handler))
        .route("/lazarus/snapshots", get(|| async { Json(crate::lazarus_vault::LazarusVault::list_snapshots()) }))
        .layer(middleware::from_fn(security::sovereign_auth_guard));

    let app = Router::new()
        .route("/api/auth/login",     post(login_handler))
        .route("/api/auth/bootstrap", get(bootstrap_handler))
        .nest("/api", api_routes)
        .route("/ws/telemetry", get(ws_bridge::ws_telemetry_handler))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap_or_else(|e| {
        eprintln!("!! [FATAL] Port 3000 already in use: {}\n   Fix: fuser -k 3000/tcp", e);
        std::process::exit(1);
    });
    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK BRIDGE LIVE: http://localhost:3000\x1b[0m");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };
    match Assets::get(&final_path) {
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&final_path).first_or_octet_stream().as_ref())], content.data).into_response(),
        None => match Assets::get("index.html") {
            Some(content) => ([(header::CONTENT_TYPE, "text/html")], content.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}
