use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::env;
use std::time::Duration;

/// Single source of truth for the embedding space. Must match the gateway model.
pub const EMBEDDING_DIM: usize = 768;
pub const EMBEDDING_MODEL: &str = "granite-embedding-311m-r2";

pub async fn get_embedding(_api_key: &str, text: &str) -> Result<Vec<f32>> {
    // The Edge gateway is the authoritative embedding provider.
    // Do not silently mix embedding models: all persisted/query vectors
    // must remain in the same EMBEDDING_DIM-dimensional vector space.
    let edge_url = env::var("NEXUS_EMBEDDING_URL").or_else(|_| {
        env::var("NEXUS_EMBEDDING_GATEWAY")
            .map(|g| format!("{}/embeddings", g.trim_end_matches('/')))
    })?;

    let model =
        env::var("NEXUS_EMBEDDING_MODEL").unwrap_or_else(|_| "granite-311m-edge".to_string());

    let client = Client::builder()
        .connect_timeout(Duration::from_millis(400))
        .timeout(Duration::from_secs(5))
        .build()?;
    let payload = json!({
        "input": text,
        "model": model
    });

    let res = client.post(&edge_url).json(&payload).send().await.map_err(|e| {
        anyhow::anyhow!("Embedding Edge gateway request failed at {}: {}", edge_url, e)
    })?;

    let status = res.status();
    let body: serde_json::Value = res.json().await.map_err(|e| {
        anyhow::anyhow!("Embedding Edge gateway returned invalid JSON (HTTP {}): {}", status, e)
    })?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Embedding Edge gateway rejected request (HTTP {}): {}",
            status,
            body
        ));
    }

    let arr = body.pointer("/data/0/embedding").and_then(|v| v.as_array()).ok_or_else(|| {
        anyhow::anyhow!("Embedding Edge gateway response missing data[0].embedding: {}", body)
    })?;

    let vector: Vec<f32> = arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();

    if vector.len() != EMBEDDING_DIM {
        return Err(anyhow::anyhow!(
            "Embedding dimension mismatch: expected {}, got {}",
            EMBEDDING_DIM,
            vector.len()
        ));
    }

    Ok(vector)
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
