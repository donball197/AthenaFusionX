use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::Response,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::time::{interval, Duration};

#[derive(Clone, Serialize)]
pub struct UnifiedTelemetry {
    pub type_id: &'static str,
    pub status: &'static str,
    pub active_vmos: u32,
    pub next_handle: String,
    pub resonance: f32,
    pub cpu_load: f32,
    pub ram_gb: f64,
    pub total_ram_gb: f64,
    pub temperature: f32,
}

pub async fn ws_telemetry_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::server::AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_vmo_socket(socket, state))
}

async fn handle_vmo_socket(mut socket: WebSocket, state: Arc<crate::server::AppState>) {
    let mut rx = state.tx.subscribe();
    let mut heartbeat = interval(Duration::from_millis(800));
    let mut frame_count: u64 = 0;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                frame_count += 1;
                let vitals = {
                    let mut monitor = state.monitor.lock().await;
                    monitor.get_vitals()
                };

                // Adaptive frame drop under CPU stress
                if vitals.cpu_load > 0.90 && frame_count % 2 == 0 {
                    continue;
                }

                let (active, next) = crate::kernel::get_live_telemetry();
                let payload = UnifiedTelemetry {
                    type_id: "HEARTBEAT",
                    status: "SOVEREIGN_MODE_ACTIVE",
                    active_vmos: active,
                    next_handle: format!("0x{:x}", next),
                    resonance: state.current_resonance.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.0,
                    cpu_load: vitals.cpu_load,
                    ram_gb: vitals.ram_gb,
                    total_ram_gb: vitals.total_ram_gb,
                    temperature: vitals.temperature,
                };

                if let Ok(json) = serde_json::to_string(&payload) {
                    if socket.send(Message::Text(json.into())).await.is_err() { break; }
                }
            }

            Ok(raw_msg) = rx.recv() => {
                // ⚡ FIX: Forward the message as-is if it's already valid typed JSON.
                // Only wrap in NEURAL_STREAM envelope if it has no "type" field.
                let forward = if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_msg) {
                    if val.get("type").is_some() {
                        // Already typed (NEURAL_STREAM, EXECUTOR, NEURAL_THOUGHT, LOG_EVENT)
                        raw_msg
                    } else {
                        // Raw string — wrap it
                        serde_json::json!({"type": "NEURAL_STREAM", "payload": raw_msg}).to_string()
                    }
                } else {
                    // Not JSON — wrap as stream text
                    serde_json::json!({"type": "NEURAL_STREAM", "payload": raw_msg}).to_string()
                };

                if socket.send(Message::Text(forward.into())).await.is_err() { break; }
            }

            else => break,
        }
    }
}
