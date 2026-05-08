use nexus_substrate::memory::MemoryPalace;
use axum::{
    extract::{State, Query},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Router, Json,
};
use rust_embed::RustEmbed;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use nexus_runtime_core::shared_state::VmoState;
use serde_json::json;
use serde::Deserialize;

use crate::{ws_bridge, vitals_handler, cluster_api};

#[derive(RustEmbed)]
#[folder = "ui/dist/"] 
struct Assets;

pub struct AppState {
    pub client:            reqwest::Client,
    pub monitor:           Arc<tokio::sync::Mutex<crate::system_monitor::SystemMonitor>>,
    pub agent:             Arc<nexus_runtime_core::neural_agent::MicroAgent>,
    pub orchestrator:      Arc<nexus_runtime_core::orchestrator::Orchestrator>,
    pub memory:            Arc<tokio::sync::Mutex<MemoryPalace>>,
    pub api_key:           String,
    pub tx:                tokio::sync::broadcast::Sender<String>,
    pub arbiter:           Arc<nexus_runtime_core::arbiter_agent::ArbiterAgent>,
    pub workspace_root:    PathBuf,
    pub vmo_state:         Arc<tokio::sync::Mutex<VmoState>>,
    pub cluster:           Arc<nexus_runtime_core::cluster::ClusterMesh>,
    pub dist:              Arc<nexus_runtime_core::distributed_scheduler::DistributedScheduler>,
}

#[derive(serde::Deserialize)]
pub struct Intent {
    pub directive: Option<String>,
}

pub async fn unified_gateway(State(state): State<Arc<AppState>>, Json(intent): Json<Intent>) -> impl IntoResponse {
    let directive_str = intent.directive.unwrap_or_default();
    let s = state.clone();
    tokio::spawn(async move {
        let d = nexus_runtime_core::cli_utils::inject_workspace_dna(&directive_str).await;
        nexus_runtime_core::task_manager::send_task(
            s.orchestrator.clone(), &s.client, &s.api_key, &d, 
            s.memory.clone(), s.vmo_state.clone(), s.tx.clone(), s.dist.clone()
        ).await;
    });
    (StatusCode::OK, Json(json!({ "status": "QUEUED" }))).into_response()
}

async fn metrics_handler() -> impl IntoResponse {
    let text = nexus_runtime_core::metrics::get_metrics_text();
    ([(header::CONTENT_TYPE, "text/plain")], text)
}

#[derive(Deserialize)]
pub struct FsQuery { path: Option<String> }

fn safe_path(workspace_root: &std::path::Path, user_path: &str) -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
    if user_path.is_empty() { return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "PATH_EMPTY" })))); }
    let path = std::path::Path::new(user_path);
    if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err((StatusCode::FORBIDDEN, Json(json!({ "error": "PATH_TRAVERSAL_DENIED" }))));
    }
    Ok(workspace_root.join(path))
}

pub async fn fs_read_handler(State(state): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> impl IntoResponse {
    let user_path = q.path.unwrap_or_default();
    match safe_path(&state.workspace_root, &user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => match tokio::fs::read_to_string(&safe).await {
            Ok(content) => Json(json!({ "content": content })).into_response(),
            Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
        },
    }
}

pub async fn fs_write_handler(State(state): State<Arc<AppState>>, Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let user_path = payload["path"].as_str().unwrap_or_default();
    let content   = payload["content"].as_str().unwrap_or_default();
    match safe_path(&state.workspace_root, user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => {
            if let Some(parent) = safe.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            match tokio::fs::write(&safe, content).await {
                Ok(_) => Json(json!({ "status": "success" })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
            }
        }
    }
}

pub async fn fs_list_handler(State(state): State<Arc<AppState>>, Query(q): Query<FsQuery>) -> impl IntoResponse {
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

pub async fn start_server(state: Arc<AppState>) {
    use axum::middleware::from_fn;
    use crate::security::sovereign_auth_guard;

    // State-mutating routes require a valid JWT bearer token.
    let authed = Router::new()
        .route("/api/task",     post(unified_gateway))
        .route("/api/fs/write", post(fs_write_handler))
        .layer(from_fn(sovereign_auth_guard));

    let app = Router::new()
        .merge(authed)
        .route("/api/metrics",       get(metrics_handler))
        .route("/api/fs/read",       get(fs_read_handler))
        .route("/api/fs/list",       get(fs_list_handler))
        .route("/api/vitals",        get(vitals_handler::handle_vitals))
        .route("/api/cluster/nodes", get(cluster_api::list_nodes))
        .route("/ws/telemetry",      get(ws_bridge::ws_telemetry_handler))
        .fallback(static_handler)
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\x1b[32m>> [SERVER] Listening on http://0.0.0.0:3000\x1b[0m");
    axum::serve(listener, app).await.unwrap();
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };
    match Assets::get(&final_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&final_path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => match Assets::get("index.html") {
            Some(content) => ([(header::CONTENT_TYPE, "text/html")], content.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}
