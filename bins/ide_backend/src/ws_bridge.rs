use crate::server::AppState;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::Response,
};
use std::sync::Arc;

pub async fn ws_telemetry_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}
