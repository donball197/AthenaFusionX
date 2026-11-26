use axum::{
    extract::{Query, Request, State},
    http::{header, StatusCode, Uri},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use nexus_runtime_core::mcp::McpServer;
use nexus_runtime_core::memory_cortex::MemoryCortex;
use nexus_runtime_core::shared_state::VmoState;
use rust_embed::RustEmbed;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::{cluster_api, vitals_handler, ws_bridge};

// ⚡ FIX: Relative path from bins/ide_backend/Cargo.toml to the UI dist folder
#[derive(RustEmbed)]
#[folder = "../../ui/dist/"]
struct Assets;

pub struct AppState {
    pub client: reqwest::Client,
    pub monitor: Arc<tokio::sync::Mutex<nexus_ide_backend::system_monitor::SystemMonitor>>,
    pub orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
    pub memory: Arc<MemoryCortex>,
    pub api_key: SecretString,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub workspace_root: PathBuf,
    pub vmo_state: Arc<tokio::sync::Mutex<VmoState>>,
    pub cluster: Arc<nexus_runtime_core::cluster::ClusterMesh>,
    pub dist: Arc<nexus_runtime_core::distributed_scheduler::DistributedScheduler>,
    pub mcp: Arc<McpServer>,
}

#[derive(serde::Deserialize)]
pub struct Intent {
    pub directive: Option<String>,
}

pub async fn unified_gateway(
    State(state): State<Arc<AppState>>,
    Json(intent): Json<Intent>,
) -> impl IntoResponse {
    let directive_str = intent.directive.unwrap_or_default();
    let s = state.clone();
    tokio::spawn(async move {
        let d = nexus_runtime_core::cli_utils::inject_workspace_dna(&directive_str).await;
        let result = nexus_runtime_core::task_manager::send_task(
            s.orchestrator.clone(),
            nexus_runtime_core::task_manager::TaskContext {
                client: s.client.clone(),
                api_key: s.api_key.expose_secret().to_string(),
                memory: s.memory.clone(),
                vmo_state: s.vmo_state.clone(),
                tx: s.tx.clone(),
                dist: s.dist.clone(),
                belief_engine: Arc::new(
                    nexus_runtime_core::cognition::belief_engine::BeliefEngine::new(),
                ),
                prediction_engine: Arc::new(
                    nexus_runtime_core::cognition::prediction_engine::PredictionEngine::new(),
                ),
                reflection_engine: Arc::new(
                    nexus_runtime_core::cognition::reflection_engine::ReflectionEngine::new(),
                ),
            },
            &d,
        )
        .await;
        let _ = s.tx.send(
            serde_json::json!({
                "type": "LOG_EVENT",
                "payload": format!("[TASK RESULT] {}", result)
            })
            .to_string(),
        );
    });
    (StatusCode::OK, Json(json!({ "status": "QUEUED" }))).into_response()
}

pub async fn mcp_handler(State(state): State<Arc<AppState>>, body: String) -> impl IntoResponse {
    let response = state.mcp.handle_request(&body);
    ([(header::CONTENT_TYPE, "application/json")], response)
}

async fn metrics_handler() -> impl IntoResponse {
    let text = nexus_runtime_core::metrics::get_metrics_text();
    ([(header::CONTENT_TYPE, "text/plain")], text)
}

#[derive(Deserialize)]
pub struct FsQuery {
    path: Option<String>,
}

fn safe_path(
    workspace_root: &Path,
    user_path: &str,
) -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
    if user_path.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "PATH_EMPTY" }))));
    }
    let path = std::path::Path::new(user_path);
    if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err((StatusCode::FORBIDDEN, Json(json!({ "error": "PATH_TRAVERSAL_DENIED" }))));
    }
    Ok(workspace_root.join(path))
}

pub async fn fs_read_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FsQuery>,
) -> impl IntoResponse {
    let user_path = q.path.unwrap_or_default();
    match safe_path(&state.workspace_root, &user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => match tokio::fs::read_to_string(&safe).await {
            Ok(content) => Json(json!({ "content": content })).into_response(),
            Err(e) => {
                (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response()
            }
        },
    }
}

pub async fn fs_write_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let user_path = payload["path"].as_str().unwrap_or_default();
    let content = payload["content"].as_str().unwrap_or_default();
    match safe_path(&state.workspace_root, user_path) {
        Err(e) => e.into_response(),
        Ok(safe) => {
            if let Some(parent) = safe.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            match tokio::fs::write(&safe, content).await {
                Ok(_) => Json(json!({ "status": "success" })).into_response(),
                Err(e) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
                        .into_response()
                }
            }
        }
    }
}

pub async fn fs_list_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FsQuery>,
) -> impl IntoResponse {
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

/// Rejects any request without a valid bearer token issued by /auth/login.
/// Applied to every route in `guarded_api` -- login itself stays outside this.
async fn auth_guard(request: Request, next: Next) -> Result<impl IntoResponse, StatusCode> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    let Some(token) = token else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    match crate::auth::verify_token_detailed(token) {
        Ok(crate::auth::TokenVerdict::Valid) => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub async fn start_server(
    state: Arc<AppState>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    // ⚡ Secure API Routes behind Auth Guard
    let guarded_api = Router::new()
        .route("/task", post(unified_gateway))
        .route("/mcp", post(mcp_handler))
        .route("/metrics", get(metrics_handler))
        .route("/fs/read", get(fs_read_handler))
        .route("/fs/write", post(fs_write_handler))
        .route("/fs/list", get(fs_list_handler))
        .route("/vitals", get(vitals_handler::handle_vitals))
        .route("/cluster/nodes", get(cluster_api::list_nodes))
        .layer(middleware::from_fn(auth_guard));

    // ⚡ FIX: Unprotected API Routes (Login) + Merge Guarded Routes
    let api_routes =
        Router::new().route("/auth/login", post(crate::auth::login_handler)).merge(guarded_api);

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", api_routes)
        .route("/ws/telemetry", get(ws_bridge::ws_telemetry_handler))
        .fallback(static_handler) // ⚡ RustEmbed serves the Svelte build
        .with_state(state);

    let addr = std::env::var("NEXUS_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("[BOOT] bind {} failed: {}", addr, e);
            std::process::exit(1);
        }
    };
    tracing::info!("[BOOT] API listening on {}", addr);
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
        })
        .await
    {
        tracing::error!("[SERVER] serve failed: {}", e);
    }
    tracing::info!("[SERVER] HTTP server stopped.");
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    let final_path = if path.is_empty() { "index.html".to_string() } else { path };

    let asset = Assets::get(&final_path).or_else(|| Assets::get(&format!("./{}", final_path)));

    match asset {
        Some(content) => {
            let mime = mime_guess::from_path(&final_path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            if final_path.contains('.') && !final_path.ends_with(".html") {
                return StatusCode::NOT_FOUND.into_response();
            }
            match Assets::get("index.html").or_else(|| Assets::get("./index.html")) {
                Some(content) => {
                    ([(header::CONTENT_TYPE, "text/html")], content.data).into_response()
                }
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}
