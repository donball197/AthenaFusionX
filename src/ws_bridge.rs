use axum::{extract::ws::{Message, WebSocket, WebSocketUpgrade}, extract::State, response::Response};
use std::sync::Arc;
use crate::server::AppState;
use serde_json::json;

pub async fn ws_telemetry_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, _state: Arc<AppState>) {
    // ⚡ FIX: Send valid JSON so the client doesn't explode on JSON.parse
    let init_msg = json!({
        "type": "LOG_EVENT",
        "payload": "TELEMETRY_LINK_ESTABLISHED",
        "cls": "text-emerald-400"
    }).to_string();
    
    let _ = socket.send(Message::Text(init_msg.into())).await;
}
