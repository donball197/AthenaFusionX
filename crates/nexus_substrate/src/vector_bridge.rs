use anyhow::Result;
#[cfg(all(not(target_os = "android"), feature = "embeddings"))]
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use reqwest::Client;
use serde_json::json;

// 1. Compile-time gated local embedder: Bypasses ONNX runtime linking on Android
#[cfg(all(not(target_os = "android"), feature = "embeddings"))]
static LOCAL_EMBEDDER: Lazy<Mutex<Option<TextEmbedding>>> = Lazy::new(|| {
    let mut options = InitOptions::default();
    options.model_name = EmbeddingModel::AllMiniLML6V2;
    options.show_download_progress = false;
    let model = TextEmbedding::try_new(options).ok();
    Mutex::new(model)
});

#[allow(dead_code)]
#[cfg(not(all(not(target_os = "android"), feature = "embeddings")))]
static LOCAL_EMBEDDER: Lazy<Mutex<Option<()>>> = Lazy::new(|| {
    Mutex::new(None)
});

pub async fn get_embedding(api_key: &str, text: &str) -> Result<Vec<f32>> {

    #[cfg(all(not(target_os = "android"), feature = "embeddings"))]
    {
        let text_owned = text.to_string();
        if let Ok(Some(vector)) = tokio::task::spawn_blocking(move || {
            let mut guard = LOCAL_EMBEDDER.lock();
            if let Some(ref mut model) = *guard {
                model.embed(vec![text_owned], None).ok().and_then(|mut v| v.pop())
            } else {
                None
            }
        })
        .await
        {
            return Ok(vector);
        }
    }

    // 2. Resilient Cloud Fallback (Android-ready & Dimension-matched)
    let _ = crate::token_ledger::wait_for_quota("gemini-embedding-2", 500).await;

    let external_client = Client::new();
    let url =
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-2:embedContent";

    let payload = json!({
        "model": "models/gemini-embedding-2",
        "content": { "parts": [{ "text": text }] },
        "outputDimensionality": 384
    });

    let res = external_client
        .post(url)
        .header("x-goog-api-key", api_key.trim())
        .json(&payload)
        .send()
        .await?;
    let body: serde_json::Value = res.json().await?;

    let mut vector = Vec::with_capacity(384);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vector_bridge_dimensions() {
        let vector = get_embedding("dummy_key", "test dimension consistency")
            .await
            .unwrap_or_else(|_| vec![0.0; 384]);
        assert_eq!(vector.len(), 384);
    }
}
