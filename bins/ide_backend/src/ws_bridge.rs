use crate::server::AppState;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

pub async fn ws_telemetry_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let authorized = q
        .get("token")
        .map(|t| {
            matches!(crate::auth::verify_token_detailed(t), Ok(crate::auth::TokenVerdict::Valid))
        })
        .unwrap_or(false);

    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if socket.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!("[WS] telemetry client lagged, skipped {} messages", n);
                continue;
            }
            Err(RecvError::Closed) => break,
        }
    }
}
