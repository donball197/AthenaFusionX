//! EdgeWorker — handles registration of remote cluster nodes via HTTP POST
//! to /api/cluster/register.  Routes are wired in server.rs.
use crate::server::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use nexus_runtime_core::cluster::ClusterNode;
use std::sync::Arc;

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
