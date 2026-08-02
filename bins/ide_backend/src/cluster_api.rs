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

/// WASM cell registry — returns cells that have been synthesised and stored.
/// Cells are written to .nexus_wasm_registry.json by wasm_forge when synthesised.
pub async fn list_wasm_cells(
    _state: axum::extract::State<std::sync::Arc<crate::server::AppState>>,
) -> axum::Json<serde_json::Value> {
    let path = ".nexus_wasm_registry.json";
    let cells: Vec<serde_json::Value> = if std::path::Path::new(path).exists() {
        tokio::fs::read_to_string(path)
            .await
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        vec![]
    };
    axum::Json(serde_json::json!({ "cells": cells }))
}
