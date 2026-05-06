use crate::server::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

pub async fn handle_vitals(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut monitor = state.monitor.lock().await;
    let v: crate::system_monitor::Vitals = monitor.get_vitals();
    Json(json!({
        "cpu_load":     v.cpu_load,
        "ram_gb":       v.ram_gb,
        "total_ram_gb": v.total_ram_gb,
        "temperature":  v.temperature,
    }))
}
