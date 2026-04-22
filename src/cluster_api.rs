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
    pub temp: f32, // Now accepting thermal data from remote nodes
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
        "best_node": best.as_ref().map(|n| json!({
            "id":   n.id,
            "host": n.host,
            "port": n.port,
            "cpu":  n.cpu,
            "ram":  n.ram,
            "temp": n.temp,
        })),
        "has_capacity": best.is_some(),
    }))
}

pub async fn list_wasm_cells() -> Json<Value> {
    match crate::wasm_registry::WasmRegistry::new(".nexus_wasm_registry.db").await {
        Err(e) => Json(json!({ "error": e.to_string() })),
        Ok(reg) => match reg.list_all().await {
            Err(e) => Json(json!({ "error": e.to_string() })),
            Ok(cells) => Json(json!({ "cells": cells })),
        },
    }
}
