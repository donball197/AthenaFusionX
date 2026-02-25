use axum::{
    extract::{Json, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct ScrapeRequest {
    url: String,
}

#[derive(Serialize)]
struct ScrapeResponse {
    content: String,
}

async fn scrape(
    // AXUM RULE: State usually comes last or must match the Router's state type exactly
    State(_state): State<()>,
    Json(payload): Json<ScrapeRequest>,
) -> impl IntoResponse {
    // For now, we return a heartbeat to prove the drone is alive
    let response = ScrapeResponse {
        content: format!("Nexus Drone acknowledged URL: {}", payload.url),
    };
    Json(response)
}

#[tokio::main]
async fn main() {
    // Define the state explicitly as empty tuple ()
    let app = Router::new()
        .route("/scrape", post(scrape))
        .with_state(());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    println!("Nexus Scraper Drone ignited on port 5000");
    axum::serve(listener, app).await.unwrap();
}
