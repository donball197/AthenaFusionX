use crate::server::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn list_nodes(State(state): State<Arc<AppState>>) -> Json<Value> {
    let nodes = state.cluster.get_all_nodes();
    Json(json!({ "nodes": nodes }))
}

pub async fn cluster_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let best = state.cluster.best_node();
    Json(json!({
        "best_node": best,
        "has_capacity": best.is_some(),
    }))
}
