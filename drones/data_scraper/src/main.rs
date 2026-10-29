use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};

const CHUNK_CHARS: usize = 8_000;
const CHUNK_OVERLAP: usize = 500;

const EXTRACTION_SYSTEM_PROMPT: &str =
    "You are a strict data extraction tool. Return only the requested data in a clean format.";

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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(ErrorResponse { error: message.into() })).into_response()
}

async fn scrape(State(_state): State<()>, Json(payload): Json<ScrapeRequest>) -> impl IntoResponse {
    let client = reqwest::Client::new();

    let content = match client.get(&payload.url).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => match resp.text().await {
                Ok(text) => text,
                Err(e) => {
                    return error_response(
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to decode response body: {}", e),
                    );
                }
            },
            Err(e) => {
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to fetch URL: {}", e),
                );
            }
        },
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e));
        }
    };

    Json(ScrapeResponse { content }).into_response()
}

fn chunk_text(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let chars: Vec<char> = text.chars().collect();

    if chars.len() <= CHUNK_CHARS {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let end = (start + CHUNK_CHARS).min(chars.len());

        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);

        if end == chars.len() {
            break;
        }

        let next_start = end.saturating_sub(CHUNK_OVERLAP);

        if next_start <= start {
            break;
        }

        start = next_start;
    }

    chunks
}

async fn extract(
    State(_state): State<()>,
    Json(payload): Json<ExtractRequest>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();

    let html = match client.get(&payload.url).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => match resp.text().await {
                Ok(text) => text,
                Err(e) => {
                    return error_response(
                        StatusCode::BAD_GATEWAY,
                        format!("Failed to decode HTML response: {}", e),
                    );
                }
            },
            Err(e) => {
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to fetch HTML: {}", e),
                );
            }
        },
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, format!("Failed to fetch HTML: {}", e));
        }
    };

    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("NEXUS_AUTH_KEY"))
        .or_else(|_| std::env::var("GEMINI_API_KEY"))
        .unwrap_or_default();

    let model = match std::env::var("NEXUS_LIGHT_MODEL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration Error: NEXUS_LIGHT_MODEL is not configured",
            );
        }
    };

    let chunks = chunk_text(&html);
    let total_chunks = chunks.len();

    println!(
        ">> [EXTRACT] source={} chars={} chunks={} chunk_limit={} overlap={}",
        payload.url,
        html.chars().count(),
        total_chunks,
        CHUNK_CHARS,
        CHUNK_OVERLAP
    );

    let mut results = Vec::with_capacity(total_chunks);

    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_number = index + 1;

        println!(
            ">> [EXTRACT] Processing chunk {}/{} ({} chars)",
            chunk_number,
            total_chunks,
            chunk.chars().count()
        );

        let user_prompt = format!(
            "Extract data from this HTML based on the following prompt:\n\
             {}\n\n\
             Document chunk {}/{}.\n\
             HTML:\n{}",
            payload.prompt, chunk_number, total_chunks, chunk
        );

        let content = match nexus_model_hub::universal_provider::UniversalProvider::generate(
            &client,
            &api_key,
            &model,
            Some(EXTRACTION_SYSTEM_PROMPT),
            &user_prompt,
            0.1,
            1024,
        )
        .await
        {
            Ok(res) if !res.trim().is_empty() => res.trim().to_string(),

            Ok(_) => {
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "LLM extraction returned empty content for chunk {}/{}",
                        chunk_number, total_chunks
                    ),
                );
            }

            Err(e) => {
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "LLM extraction failed on chunk {}/{}: {}",
                        chunk_number, total_chunks, e
                    ),
                );
            }
        };

        results.push(format!(
            "=== EXTRACTION CHUNK {}/{} ===\n{}",
            chunk_number, total_chunks, content
        ));
    }

    // Normalize chunk-level extraction results before aggregation.
    //
    // A chunk may contain useful partial fields plus NO_RELEVANT_DATA
    // placeholders when the target crosses a chunk boundary.  Those
    // placeholders must not leak into the final response.
    //
    // IMPORTANT:
    // - This changes aggregation only.
    // - Chunking, overlap, prompts, fetching, and inference are untouched.
    // - Real extracted values are preserved.
    let mut normalized_results = Vec::with_capacity(results.len());

    for result in results {
        let normalized = result
            .lines()
            .filter(|line| {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    return false;
                }

                let upper = trimmed.to_ascii_uppercase();

                // Remove complete no-data markers.
                if upper == "NO_RELEVANT_DATA"
                    || upper == "- NO_RELEVANT_DATA"
                    || upper == "NO RELEVANT DATA"
                    || upper == "- NO RELEVANT DATA"
                {
                    return false;
                }

                // Remove field-level placeholders such as:
                // Value: NO_RELEVANT_DATA
                // Deployment: NO_RELEVANT_DATA
                // Link: NO_RELEVANT_DATA
                if upper.ends_with(": NO_RELEVANT_DATA")
                    || upper.ends_with(": NO RELEVANT DATA")
                    || upper.ends_with(": - NO_RELEVANT_DATA")
                    || upper.ends_with(": - NO RELEVANT DATA")
                {
                    return false;
                }

                true
            })
            .collect::<Vec<_>>()
            .join("\n");

        let normalized = normalized.trim();

        if !normalized.is_empty() {
            normalized_results.push(normalized.to_string());
        }
    }

    // Deduplicate identical normalized chunk responses while preserving order.
    let mut unique_results = Vec::with_capacity(normalized_results.len());

    for result in normalized_results {
        if !unique_results.contains(&result) {
            unique_results.push(result);
        }
    }

    let combined = if unique_results.is_empty() {
        "No matching data found in the supplied document.".to_string()
    } else {
        unique_results.join("\n\n")
    };

    println!(
        ">> [EXTRACT] Completed successfully: {}/{} chunks; {} useful results",
        total_chunks,
        total_chunks,
        unique_results.len()
    );

    Json(ScrapeResponse { content: combined }).into_response()
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let app = Router::new()
        .route("/scrape", post(scrape))
        .route("/extract", post(extract))
        .with_state(());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000")
        .await
        .expect("Failed to bind scraper listener on port 5000");

    println!(">> [DRONE] Smart Scraper ignited on port 5000");

    axum::serve(listener, app).await.expect("Smart Scraper server failed");
}
