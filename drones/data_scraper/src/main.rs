use axum::{
    extract::{Json, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ScrapeRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct ExtractRequest {
    pub url: String,
    pub prompt: String,
}

#[derive(Serialize)]
pub struct ScrapeResponse {
    pub content: String,
}

async fn scrape(State(_state): State<()>, Json(payload): Json<ScrapeRequest>) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let content = match client.get(&payload.url).send().await {
        Ok(resp) => resp.text().await.unwrap_or_else(|_| "Failed to decode body".to_string()),
        Err(e) => format!("Failed to fetch URL: {}", e),
    };
    Json(ScrapeResponse { content })
}

async fn extract(
    State(_state): State<()>,
    Json(payload): Json<ExtractRequest>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let html = match client.get(&payload.url).send().await {
        Ok(resp) => resp.text().await.unwrap_or_default(),
        Err(_) => return Json(ScrapeResponse { content: "Failed to fetch HTML".to_string() }),
    };

    let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
    let safe_html: String = html.chars().take(20000).collect(); // Prevent context overflow
    let user_prompt = format!(
        "Extract data from this HTML based on the following prompt:\n{}\n\nHTML:\n{}",
        payload.prompt, safe_html
    );

    let content = match nexus_model_hub::universal_provider::UniversalProvider::generate(
        &client, &api_key, "gemini-3.5-flash-lite", Some("You are a strict data extraction tool. Return only the requested data in a clean format."), &user_prompt, 0.1, 4096
    ).await {
        Ok(res) => res,
        Err(e) => format!("LLM Extraction Error: {}", e),
    };

    Json(ScrapeResponse { content })
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let app = Router::new()
        .route("/scrape", post(scrape))
        .route("/extract", post(extract))
        .with_state(());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    println!(">> [DRONE] Smart Scraper ignited on port 5000");
    axum::serve(listener, app).await.unwrap();
}
