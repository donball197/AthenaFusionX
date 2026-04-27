// src/server.rs — v4.8.2 [CLEANUP_COMPLETE]
use axum::{
    extract::{State, Query},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router, middleware,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tokio::net::TcpListener;

use crate::{vitals_handler, ws_bridge, logic_engine::LogicEngine, security, cluster_api, auth};

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

pub struct AppState {
    pub client: reqwest::Client,
    pub monitor: Arc<tokio::sync::Mutex<crate::system_monitor::SystemMonitor>>,
    pub agent: Arc<crate::neural_agent::MicroAgent>,
    pub orchestrator: Arc<crate::orchestrator::Orchestrator>,
    pub memory: Arc<tokio::sync::Mutex<crate::memory::MemoryPalace>>,
    pub api_key: String,
    pub current_resonance: Arc<std::sync::atomic::AtomicU32>,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub scheduler_tx: tokio::sync::mpsc::Sender<crate::scheduler::NexusTask>,
    pub arbiter: Arc<crate::arbiter_agent::ArbiterAgent>,
    pub workspace_root: std::path::PathBuf,
    pub vmo_state: Arc<tokio::sync::Mutex<crate::server::VmoState>>,
    pub cluster: Arc<crate::cluster::ClusterMesh>,
    pub dist: Arc<crate::distributed_scheduler::DistributedScheduler>,
    pub governance_log: Arc<parking_lot::Mutex<crate::governance::protocol::GovernanceLog>>,
}

#[derive(Default, Debug)]
pub struct VmoState {
    pub last_parent: Option<u32>,
    pub last_child: Option<u32>,
}

#[derive(Deserialize)]
pub struct Intent {
    pub intent_type: Option<String>,
    pub directive: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct LoginRequest { key: String }

#[derive(Deserialize)]
struct FsQuery { path: Option<String> }

async fn login_handler(Json(payload): Json<LoginRequest>) -> impl IntoResponse {
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

async fn unified_gateway(State(state): State<Arc<AppState>>, Json(intent): Json<Intent>) -> impl IntoResponse {
    let directive_str = intent.directive.or_else(|| intent.payload.as_ref()?.get("directive")?.as_str().map(|s| s.to_string())).unwrap_or_default();
    if !LogicEngine::is_action_safe(&directive_str) {
        return (StatusCode::FORBIDDEN, Json(json!({"status": "LOGIC_VIOLATION"}))).into_response();
    }

    let s = state.clone(); let d_clone = directive_str.clone();
    tokio::spawn(async move {
        let d = crate::cli::inject_workspace_dna(&d_clone).await;
        crate::task_manager::send_task(s.orchestrator.clone(), &s.client, &s.api_key, &d, s.memory.clone(), s.vmo_state.clone(), s.tx.clone(), s.dist.clone()).await;
    });
    (StatusCode::OK, Json(json!({ "status": "QUEUED" }))).into_response()
}

async fn metrics_handler() -> impl IntoResponse {
    let metrics_text = crate::metrics::get_metrics_text();
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], metrics_text)
}

pub async fn start_server(state: Arc<AppState>) {
    let api_routes = Router::new()
        .route("/task", post(unified_gateway))
        .route("/intent", post(unified_gateway))
        .route("/vmos", get(crate::vmo_handler::handle_get_vmos))
        .route("/vitals", get(vitals_handler::handle_vitals))
        .route("/metrics", get(metrics_handler))
        .route("/cluster/nodes", get(cluster_api::list_nodes))
        .route("/cluster/forge", post(crate::edge_worker::run_remote_synthesis))
        .route("/cluster/run", post(crate::edge_worker::run_remote_job))
        .route("/fs/list", get(|Query(q): Query<FsQuery>| async move {
            let path = q.path.unwrap_or_else(|| ".".into());
            let mut entries = Vec::new();
            if let Ok(mut read_dir) = tokio::fs::read_dir(&path).await {
                while let Ok(Some(entry)) = read_dir.next_entry().await {
                    let meta = entry.metadata().await.ok();
                    entries.push(serde_json::json!({
                        "name": entry.file_name().to_string_lossy(),
                        "path": entry.path(),
                        "is_dir": meta.map(|m| m.is_dir()).unwrap_or(false)
                    }));
                }
            }
            Json(entries)
        }))
        .route("/lazarus/snapshots", get(move || async { Json(crate::lazarus_vault::LazarusVault::list_snapshots()) }))
        .layer(middleware::from_fn(security::sovereign_auth_guard));

    let app = Router::new()
        .route("/api/auth/login", post(login_handler))
        .nest("/api", api_routes)
        .route("/ws/telemetry", get(ws_bridge::ws_telemetry_handler))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\x1b[38;2;0;255;255m>> [NEXUS] UPLINK BRIDGE LIVE: http://localhost:3000\x1b[0m");
    axum::serve(listener, app).await.unwrap();
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };
    match Assets::get(&final_path) {
        Some(content) => ([(header::CONTENT_TYPE, mime_guess::from_path(&final_path).first_or_octet_stream().as_ref())], content.data).into_response(),
        None => match Assets::get("index.html") {
            Some(content) => ([(header::CONTENT_TYPE, "text/html")], content.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}
