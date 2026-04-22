// src/cluster_api.rs — v1.0 Cluster Management API
// Provides HTTP endpoints for cluster node registration and status.
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::cluster::{ClusterNode};
use crate::server::AppState;

#[derive(Deserialize)]
pub struct RegisterPayload {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub cpu: f32,
    pub ram: f32,
}

#[derive(Serialize)]
pub struct ClusterStatus {
    pub node_count: usize,
    pub best_node_id: Option<String>,
    pub best_node_cpu: Option<f32>,
    pub best_node_ram: Option<f32>,
}

/// POST /api/cluster/register — A remote node announces itself.
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
        last_seen: 0, // ClusterMesh::register sets this
    });
    Json(json!({ "ok": true, "registered": payload.id }))
}

/// GET /api/cluster/status — Returns cluster health summary.
pub async fn cluster_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Count alive nodes (ClusterMesh filters stale ones in best_node)
    let best = state.cluster.best_node();
    Json(json!({
        "best_node": best.as_ref().map(|n| json!({
            "id":   n.id,
            "host": n.host,
            "port": n.port,
            "cpu":  n.cpu,
            "ram":  n.ram,
        })),
        "has_capacity": best.is_some(),
    }))
}

/// GET /api/cluster/wasm_registry — List all stored WASM cells.
pub async fn list_wasm_cells() -> Json<Value> {
    match crate::wasm_registry::WasmRegistry::new(".nexus_wasm_registry.db").await {
        Err(e) => Json(json!({ "error": e.to_string() })),
        Ok(reg) => match reg.list_all().await {
            Err(e) => Json(json!({ "error": e.to_string() })),
            Ok(cells) => Json(json!({ "cells": cells })),
        },
    }
}
