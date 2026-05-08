use anyhow::Result;
use reqwest::Client;
use serde_json::json;

pub async fn get_embedding(api_key: &str, text: &str) -> Result<Vec<f32>> {
    // wait_for_quota is an async fn that returns ()
    let _ = crate::token_ledger::wait_for_quota("gemini-embedding-2", 500).await;

    let client = Client::new();
    
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-2:embedContent?key={}",
        api_key
    );

    let payload = json!({
        "model": "models/gemini-embedding-2",
        "content": { "parts": [{ "text": text }] }
    });

    let res = client.post(&url).json(&payload).send().await?;
    let body: serde_json::Value = res.json().await?;

    let mut vector = Vec::new();
    if let Some(values) = body.pointer("/embedding/values").and_then(|v| v.as_array()) {
        for v in values {
            if let Some(f) = v.as_f64() {
                vector.push(f as f32);
            }
        }
    } else {
        return Err(anyhow::anyhow!("API REJECTED EMBEDDING. Raw response: {}", body));
    }

    Ok(vector)
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
    dot / (norm_a * norm_b)
}
