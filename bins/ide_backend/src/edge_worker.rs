//! EdgeWorker — handles registration of remote cluster nodes via HTTP POST
//! to /api/cluster/register.  Routes are wired in server.rs.
use axum::{extract::State, Json, response::IntoResponse};
use std::sync::Arc;
use crate::server::AppState;
use nexus_runtime_core::cluster::ClusterNode;

pub fn init() {
    tracing::info!(">> [EDGE_WORKER] Peer registration handler online");
}

pub async fn register_node(
    State(state): State<Arc<AppState>>,
    Json(node): Json<ClusterNode>,
) -> impl IntoResponse {
    state.cluster.register(node.clone());
    tracing::info!("[EDGE_WORKER] Node {} registered at {}:{}", node.id, node.host, node.port);
    Json(serde_json::json!({ "status": "registered", "id": node.id }))
}
