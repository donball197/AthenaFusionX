use axum::{
    routing::{get, post},
    extract::State,
    Json, Router,
};
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct PromptRequest {
    pub prompt: String,
}

#[derive(Deserialize, Serialize)]
pub struct NeuralResponse {
    pub response: String,
    pub status: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/prompt", post(handle_prompt))
        .with_state(state)
}

async fn root() -> &'static str {
    ">> [NEXUS] SYSTEM ONLINE. SWARM READY."
}

async fn handle_prompt(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PromptRequest>,
) -> Json<NeuralResponse> {
    let client = &state.client;
    let endpoint = &state.brain_endpoint;

    let res = client.post(format!("{}/inference", endpoint))
        .json(&payload)
        .send()
        .await;

    match res {
        Ok(response) => {
            let data: NeuralResponse = response.json().await.unwrap_or(NeuralResponse {
                response: "Brain communication failure.".to_string(),
                status: "error".to_string(),
            });
            Json(data)
        },
        Err(_) => Json(NeuralResponse {
            response: "Brain Agent Offline.".to_string(),
            status: "error".to_string(),
        }),
    }
}
