//! Episodic strategy retrieval over the local granite edge embedder.
//!
//! Backs MEMORY_RECALL. Given an objective and the current contract signature,
//! returns the top-k prior strategies ranked by weighted cosine similarity and
//! outcome polarity. The embedder endpoint defaults to the verified live
//! granite-311m-edge at http://127.0.0.1:9380/v1/embeddings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub objective_embedding: Vec<f32>, // 768-dim, L2-normalized
    pub contract_signature: String,
    pub chosen_strategy: String,
    pub outcome: EpisodeOutcome,
    pub node_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum EpisodeOutcome {
    Completed,
    HaltRejected,
    Stalled,
    Vetoed,
}

/// Order-insensitive, Debug-form based hash of a condition set. Stable across
/// runs so the same contract always maps to the same retrieval bucket.
pub fn contract_signature(debug_forms: &[String]) -> String {
    let mut parts = debug_forms.to_vec();
    parts.sort();
    parts.dedup();
    crate::journal::hash_bytes(parts.join("\n").as_bytes())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    // assumes L2-normalized
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn l2_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in &mut v {
            *x /= n;
        }
    }
    v
}

/// Retrieve the top-k prior strategies for this objective, preferring episodes
/// recorded under the same contract signature and weighting successful
/// outcomes above failed ones.
pub async fn suggest(
    client: &reqwest::Client,
    embed_endpoint: &str,
    objective: &str,
    signature: &str,
    corpus: &[Episode],
    k: usize,
) -> anyhow::Result<Vec<(String, f32)>> {
    if corpus.is_empty() || k == 0 {
        return Ok(Vec::new());
    }

    let resp: serde_json::Value = client
        .post(embed_endpoint)
        .json(&serde_json::json!({ "input": objective, "model": "granite-311m-edge" }))
        .send()
        .await?
        .json()
        .await?;

    let raw: Vec<f32> = resp["data"][0]["embedding"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("embedder returned no embedding"))?
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();
    let q = l2_normalize(raw);

    let mut scored: Vec<(f32, &Episode)> = corpus
        .iter()
        .map(|e| {
            let sim = cosine(&q, &e.objective_embedding);
            let bonus = if e.contract_signature == signature { 0.15 } else { 0.0 };
            (sim + bonus, e)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut tally: std::collections::HashMap<&str, (f32, u32)> = Default::default();
    for (s, e) in scored.iter().take(k * 4) {
        let weight = if e.outcome == EpisodeOutcome::Completed { 1.0 } else { -0.5 };
        let t = tally.entry(e.chosen_strategy.as_str()).or_insert((0.0, 0));
        t.0 += s * weight;
        t.1 += 1;
    }

    let mut out: Vec<(String, f32)> =
        tally.into_iter().map(|(s, (score, n))| (s.to_string(), score / n as f32)).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.truncate(k);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_order_insensitive() {
        let a = vec!["ReadMarkerAcquired".to_string(), "HaltDelivered".to_string()];
        let b = vec!["HaltDelivered".to_string(), "ReadMarkerAcquired".to_string()];
        assert_eq!(contract_signature(&a), contract_signature(&b));
    }

    #[test]
    fn l2_normalize_unit_length() {
        let v = l2_normalize(vec![3.0, 4.0]);
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-5);
    }
}
