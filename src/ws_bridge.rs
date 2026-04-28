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
    
    // Send the initial handshake
    if socket.send(Message::Text(init_msg.into())).await.is_err() {
        return;
    }

    // Subscribe to the global matrix broadcast channel
    let mut rx = state.tx.subscribe();

    // Trap the socket in a continuous multiplexing loop
    loop {
        tokio::select! {
            // 1. Forward internal Cortex broadcasts to the Svelte frontend
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break; // Connection severed by browser
                }
            }
            // 2. Listen for frontend telemetry (Ghost DOM) or disconnects
            client_msg = socket.recv() => {
                if let Some(Ok(Message::Text(txt))) = client_msg {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if json["type"] == "GHOST_DOM_STATE" {
                            // Inject the UI state directly into the tracing layer
                            // which forwards it into the agent's context buffer
                            tracing::info!("[GHOST_DOM] Frontend State: {}", json["payload"]);
                        }
                    }
                } else if client_msg.is_none() {
                    break; // Browser closed the tab/socket
                }
            }
        }
    }
}
