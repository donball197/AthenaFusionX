use axum::{
    extract as ax_extract,
    routing::{get, post},
    response::Html,
    Json,
    Router
};
use std::sync::Arc;
use tower_http::services::ServeDir;
use crate::AppState;
use crate::resonator::Resonator;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/vitals", get(get_vitals))
        .route("/api/mission", post(run_mission))
        .route("/api/flush", post(flush_memory))
        .route("/api/wake", post(wake_swarm))
        .fallback_service(ServeDir::new("ui/dist"))
        .with_state(state)
}

async fn index() -> Html<String> {
    match std::fs::read_to_string("ui/dist/index.html") {
        Ok(content) => Html(content),
        Err(_) => Html("<h1>NEXUS_IDE: UI_NOT_COMPILED</h1>".to_string()),
    }
}

async fn get_vitals(ax_extract::State(state): ax_extract::State<Arc<AppState>>) -> Json<serde_json::Value> {
    let swarm = state.swarm.lock().await;
    let active_count = swarm.iter().filter(|a| a.is_active).count();

    let agent_statuses: Vec<serde_json::Value> = swarm.iter().map(|a| {
        serde_json::json!({
            "id": a.id,
            "hive": a.hive_id,
            "target": a.current_target,
            "is_active": a.is_active
        })
    }).collect();

    Json(serde_json::json!({
        "status": if active_count == 68 { "BATTLE_STATIONS" } else { "OPERATIONAL" },
        "ram_usage": format!("{}MB", 150 + (active_count * 15)),
        "swarm": agent_statuses
    }))
}

async fn wake_swarm(ax_extract::State(state): ax_extract::State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut swarm = state.swarm.lock().await;
    for agent in swarm.iter_mut() { agent.ignite(); }
    Json(serde_json::json!({"status": "SWARM_AWAKENED"}))
}

async fn flush_memory(ax_extract::State(state): ax_extract::State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut swarm = state.swarm.lock().await;
    for agent in swarm.iter_mut() { agent.clear_scratchpad(); }
    Json(serde_json::json!({"status": "COLD_STANDBY"}))
}

async fn run_mission(
    ax_extract::State(state): ax_extract::State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>
) -> Json<serde_json::Value> {
    let mut swarm = state.swarm.lock().await;
    let _goal = payload["goal"].as_str().unwrap_or("Audit");

    let muscle_report = Resonator::scan_vault("vault/source_code").unwrap_or_else(|_| "Vault silent.".into());

    for (i, agent) in swarm.iter_mut().enumerate().take(68) {
        if i % 4 == 0 { 
            agent.ignite();
            agent.execute_mission(&format!("SCANNING_SECTOR_{}", i));
        }
    }

    Json(serde_json::json!({
        "mission_status": "SUCCESS",
        "echo": muscle_report
    }))
}
