use axum::{extract::State, Json};
use serde::{Deserialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::cluster::ClusterNode;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct RegisterPayload {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub cpu: f32,
    pub ram: f32,
    pub temp: f32,
}

pub async fn register_node(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterPayload>,
) -> Json<Value> {
    state.cluster.register(ClusterNode {
        id: payload.id.clone(),
        host: payload.host,
        port: payload.port,
        cpu: payload.cpu,
        ram: payload.ram,
        temp: payload.temp,
        last_seen: 0,
    });
    Json(json!({ "ok": true, "registered": payload.id }))
}

pub async fn cluster_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let best = state.cluster.best_node();
    Json(json!({
        "best_node": best,
        "has_capacity": best.is_some(),
    }))
}

// ⚡ NEW: Expose every node in the mesh for the UI SwarmMap
pub async fn list_nodes(State(state): State<Arc<AppState>>) -> Json<Value> {
    let nodes = state.cluster.get_all_nodes();
    Json(json!({ "nodes": nodes }))
}
