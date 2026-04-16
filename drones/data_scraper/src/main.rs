/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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
