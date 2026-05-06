// src/ws_bridge.rs — v2.0 [GHOST_DOM_ENABLED]
use axum::{extract::ws::{Message, WebSocket, WebSocketUpgrade}, extract::State, response::Response};
use std::sync::Arc;
use crate::server::AppState;
use serde_json::json;

pub async fn ws_telemetry_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let init_msg = json!({
        "type": "LOG_EVENT",
        "payload": "TELEMETRY_LINK_ESTABLISHED",
        "cls": "text-emerald-400"
    }).to_string();
    
    // Explicit construction to avoid inference ambiguity
    if socket.send(Message::Text(init_msg.into())).await.is_err() {
        return;
    }

    let mut rx = state.tx.subscribe();

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                let text_payload: String = msg;
                if socket.send(Message::Text(text_payload.into())).await.is_err() {
                    break;
                }
            }
            client_msg = socket.recv() => {
                if let Some(Ok(Message::Text(txt))) = client_msg {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if json["type"] == "GHOST_DOM_STATE" {
                            tracing::info!("[GHOST_DOM] Frontend State: {}", json["payload"]);
                        }
                    }
                } else if client_msg.is_none() {
                    break; 
                }
            }
        }
    }
}
