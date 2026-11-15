//! Tier 2 semantic support check.
//!
//! Embeds the report and the evidence corpus using the local granite
//! embedding endpoint (default `http://127.0.0.1:9380/v1/embeddings`,
//! model alias `granite-311m-edge`) and asserts that report chunks are, on
//! average and in coverage, semantically supported by the corpus.
//!
//! Scope: this check catches UNGROUNDED PROSE. It does NOT catch a
//! confidently worded false claim that is semantically close to the
//! evidence, and it does NOT catch an unsafe action plan.
//!
//! Fail-open. A dead endpoint, timeout, malformed response, or embedding
//! count mismatch all produce an empty defect list and a tracing::warn!.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:9380/v1/embeddings";
const DEFAULT_MODEL: &str = "granite-311m-edge";
const REQUEST_TIMEOUT_SECS: u64 = 5;

const MIN_CHUNK_BYTES: usize = 30;
const MAX_CHUNK_BYTES: usize = 500;

const MEAN_SUPPORT_FLOOR: f64 = 0.62;
const COVERAGE_FLOOR: f64 = 0.68;
const PER_CHUNK_SUPPORT_FLOOR: f64 = 0.60;
const EMBED_BATCH_SIZE_DEFAULT: usize = 8;

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [&'a str],
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

struct CacheKey {
    corpus_hash: u64,
    endpoint: String,
    model: String,
}

// No once_cell: plain static Mutex, initialized empty.
static CACHE: Mutex<Option<(CacheKey, Vec<Vec<f32>>)>> = Mutex::const_new(None);

fn hash_corpus(corpus: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    corpus.hash(&mut hasher);
    hasher.finish()
}

fn endpoint() -> String {
    std::env::var("NEXUS_EMBED_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string())
}

fn model() -> String {
    std::env::var("NEXUS_EMBED_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

fn embed_batch_size() -> usize {
    std::env::var("NEXUS_EMBED_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(EMBED_BATCH_SIZE_DEFAULT)
}

fn disabled() -> bool {
    // Fail safe: semantic support is opt-in until its thresholds are calibrated.
    // Explicitly enable with NEXUS_SEMANTIC_SUPPORT=1.
    match std::env::var("NEXUS_SEMANTIC_SUPPORT") {
        Ok(v) => {
            let value = v.trim();
            !(value == "1"
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
                || value.eq_ignore_ascii_case("on"))
        }
        Err(_) => true,
    }
}

/// Production entry point. Configuration is read once per invocation.
pub async fn semantic_support_defects(report: &str, corpus: &str) -> Vec<String> {
    let endpoint = endpoint();
    let model = model();
    let disabled = disabled();

    semantic_support_defects_with(report, corpus, &endpoint, &model, disabled).await
}

/// Metrics are returned independently of acceptance thresholds so
/// calibration can inspect both passing and failing reports.
#[derive(Debug, Clone)]
struct SupportMetrics {
    mean_support: f64,
    coverage: f64,
    weakest_support: f64,
    weakest_preview: String,
}

/// Parameterized production implementation.
pub(crate) async fn semantic_support_defects_with(
    report: &str,
    corpus: &str,
    endpoint: &str,
    model: &str,
    disabled: bool,
) -> Vec<String> {
    if disabled {
        return Vec::new();
    }

    let metrics = match semantic_support_metrics_with(report, corpus, endpoint, model).await {
        Some(metrics) => metrics,
        None => return Vec::new(),
    };

    if metrics.mean_support >= MEAN_SUPPORT_FLOOR && metrics.coverage >= COVERAGE_FLOOR {
        return Vec::new();
    }

    vec![format!(
        "Semantic support below threshold: mean_support={:.3} (floor {:.2}), \
         coverage={:.3} (floor {:.2}). \
         Weakest chunk (support {:.3}): {:?}",
        metrics.mean_support,
        MEAN_SUPPORT_FLOOR,
        metrics.coverage,
        COVERAGE_FLOOR,
        metrics.weakest_support,
        metrics.weakest_preview,
    )]
}

/// Computes scores without applying acceptance thresholds.
/// None means measurement failed; production callers remain fail-open.
async fn semantic_support_metrics_with(
    report: &str,
    corpus: &str,
    endpoint: &str,
    model: &str,
) -> Option<SupportMetrics> {
    semantic_support_metrics_with_timeout(
        report,
        corpus,
        endpoint,
        model,
        Duration::from_secs(REQUEST_TIMEOUT_SECS),
    )
    .await
}

async fn semantic_support_metrics_with_timeout(
    report: &str,
    corpus: &str,
    endpoint: &str,
    model: &str,
    timeout: Duration,
) -> Option<SupportMetrics> {
    let report_chunks = chunk(report);
    let corpus_chunks = chunk(corpus);

    if report_chunks.is_empty() || corpus_chunks.is_empty() {
        tracing::warn!(
            report_chunks = report_chunks.len(),
            corpus_chunks = corpus_chunks.len(),
            "semantic support: empty report or corpus after chunking; skipping"
        );
        return None;
    }

    let client = match Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "semantic support: client build failed; skipping");
            return None;
        }
    };

    let corpus_vecs = match embed_cached(&client, corpus, &corpus_chunks, endpoint, model).await {
        Some(vectors) => vectors,
        None => return None,
    };

    let report_vecs = match embed(&client, &report_chunks, endpoint, model).await {
        Some(vectors) => vectors,
        None => return None,
    };

    if corpus_vecs.len() != corpus_chunks.len() {
        tracing::warn!(
            expected = corpus_chunks.len(),
            got = corpus_vecs.len(),
            "semantic support: corpus embedding count mismatch; skipping"
        );
        return None;
    }

    if report_vecs.len() != report_chunks.len() {
        tracing::warn!(
            expected = report_chunks.len(),
            got = report_vecs.len(),
            "semantic support: report embedding count mismatch; skipping"
        );
        return None;
    }

    let supports: Vec<f64> = report_vecs
        .iter()
        .map(|report_vector| {
            corpus_vecs
                .iter()
                .map(|corpus_vector| cosine(report_vector, corpus_vector))
                .fold(0.0f32, f32::max) as f64
        })
        .collect();

    if supports.is_empty() {
        return None;
    }

    let count = supports.len() as f64;
    let mean_support = supports.iter().sum::<f64>() / count;
    let covered = supports.iter().filter(|score| **score >= PER_CHUNK_SUPPORT_FLOOR).count() as f64;
    let coverage = covered / count;

    let (weakest_idx, weakest_support) = supports
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(index, score)| (index, *score))
        .unwrap_or((0, 0.0));

    let preview_src = &report_chunks[weakest_idx];
    let preview_end =
        preview_src.char_indices().nth(120).map(|(index, _)| index).unwrap_or(preview_src.len());

    Some(SupportMetrics {
        mean_support,
        coverage,
        weakest_support,
        weakest_preview: preview_src[..preview_end].to_string(),
    })
}

async fn embed_cached(
    client: &Client,
    corpus: &str,
    chunks: &[String],
    endpoint: &str,
    model: &str,
) -> Option<Vec<Vec<f32>>> {
    let key = CacheKey {
        corpus_hash: hash_corpus(corpus),
        endpoint: endpoint.to_string(),
        model: model.to_string(),
    };
    {
        let guard = CACHE.lock().await;
        if let Some((cached, vecs)) = guard.as_ref() {
            if cached.corpus_hash == key.corpus_hash
                && cached.endpoint == key.endpoint
                && cached.model == key.model
            {
                return Some(vecs.clone());
            }
        }
    }
    let vecs = embed(client, chunks, endpoint, model).await?;
    let mut guard = CACHE.lock().await;
    *guard = Some((key, vecs.clone()));
    Some(vecs)
}

async fn embed(
    client: &Client,
    chunks: &[String],
    endpoint: &str,
    model: &str,
) -> Option<Vec<Vec<f32>>> {
    let batch_size = embed_batch_size();
    let url = endpoint.to_string();
    let mut all = Vec::with_capacity(chunks.len());
    for batch in chunks.chunks(batch_size) {
        let inputs: Vec<&str> = batch.iter().map(String::as_str).collect();
        let body = EmbeddingRequest { model, input: &inputs };
        let resp = match client.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, url = %url, batch_size, "semantic support: request failed; skipping");
                return None;
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), url = %url, batch_size, "semantic support: non-2xx; skipping");
            return None;
        }
        let parsed = match resp.json::<EmbeddingResponse>().await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "semantic support: decode failed; skipping");
                return None;
            }
        };
        for item in parsed.data {
            all.push(normalize(item.embedding));
        }
    }
    Some(all)
}

fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
    }
    dot
}

fn chunk(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let trimmed = block.trim();
        if trimmed.len() < MIN_CHUNK_BYTES {
            continue;
        }
        if trimmed.len() <= MAX_CHUNK_BYTES {
            out.push(trimmed.to_string());
            continue;
        }
        let mut current = String::new();
        for sentence in split_sentences(trimmed) {
            for piece in split_at_char_boundary(sentence, MAX_CHUNK_BYTES) {
                if piece.trim().len() < MIN_CHUNK_BYTES {
                    continue;
                }
                if !current.is_empty() && current.len() + piece.len() + 1 > MAX_CHUNK_BYTES {
                    if current.trim().len() >= MIN_CHUNK_BYTES {
                        out.push(current.trim().to_string());
                    }
                    current.clear();
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(piece);
            }
        }
        if current.trim().len() >= MIN_CHUNK_BYTES {
            out.push(current.trim().to_string());
        }
    }
    out
}

fn split_at_char_boundary(text: &str, max_bytes: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + max_bytes).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            break;
        }
        out.push(&text[start..end]);
        start = end;
    }
    out
}

fn split_sentences(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..bytes.len() {
        let c = bytes[i];
        if (c == b'.' || c == b'?' || c == b'!') && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
            out.push(&text[start..=i]);
            start = i + 1;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fail_open_cases() {
        let disabled_result = semantic_support_defects_with(
            "anything",
            "anything",
            "http://127.0.0.1:1/v1/embeddings",
            "granite-311m-edge",
            true,
        )
        .await;

        assert!(disabled_result.is_empty(), "disabled must fail open");

        let endpoint_result = semantic_support_defects_with(
            "This is a substantive sentence about the workspace.",
            "The workspace manifest lists members.",
            "http://127.0.0.1:1/v1/embeddings",
            "granite-311m-edge",
            false,
        )
        .await;

        assert!(
            endpoint_result.is_empty(),
            "dead endpoint must fail open, got: {endpoint_result:?}"
        );
    }

    #[test]
    fn cosine_of_normalized_unit_vectors_is_one() {
        let a = normalize(vec![1.0, 2.0, 3.0]);
        let b = normalize(vec![1.0, 2.0, 3.0]);
        let s = cosine(&a, &b);
        assert!((s - 1.0).abs() < 1e-5, "got {s}");
    }

    #[test]
    fn chunk_respects_max_and_min() {
        assert!(chunk("tiny").is_empty());
        let long = "a".repeat(1200);
        let chunks = chunk(&long);
        assert!(!chunks.is_empty());
        for c in &chunks {
            assert!(c.len() <= MAX_CHUNK_BYTES, "chunk too large: {}", c.len());
        }
    }

    #[test]
    fn chunk_handles_multibyte_without_panic() {
        let long: String = "\u{65e5}".repeat(400);
        let chunks = chunk(&long);
        assert!(!chunks.is_empty());
        for c in &chunks {
            assert!(c.len() <= MAX_CHUNK_BYTES, "chunk too large: {}", c.len());
        }
    }

    async fn mock_embed_server(grounded: bool) -> (String, tokio::task::JoinHandle<usize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::{TcpListener, TcpStream};

        async fn read_body(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
            let mut headers = Vec::new();
            let mut byte = [0u8; 1];

            loop {
                stream.read_exact(&mut byte).await?;
                headers.push(byte[0]);

                if headers.ends_with(b"\r\n\r\n") {
                    break;
                }

                if headers.len() > 32 * 1024 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "HTTP headers too large",
                    ));
                }
            }

            let header_text = String::from_utf8_lossy(&headers);
            let content_length = header_text
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.trim().eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "missing Content-Length")
                })?;

            let mut body = vec![0u8; content_length];
            stream.read_exact(&mut body).await?;
            Ok(body)
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock embedding server");
        let address = listener.local_addr().expect("mock server address");
        let endpoint = format!("http://{address}/v1/embeddings");

        let task = tokio::spawn(async move {
            let mut served = 0usize;

            // One request embeds the corpus; the other embeds the report.
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.expect("accept request");
                let body = read_body(&mut stream).await.expect("read request body");
                let request: serde_json::Value =
                    serde_json::from_slice(&body).expect("decode embedding request");

                assert_eq!(request["model"], "mock-model");

                let inputs = request["input"].as_array().expect("embedding input must be an array");

                assert!(!inputs.is_empty(), "embedding input must not be empty");

                let is_report = inputs.iter().any(|input| {
                    input.as_str().map(|text| text.contains("REPORT_MARKER")).unwrap_or(false)
                });

                let mut embedding = vec![0.0f32; 768];

                if grounded || is_report {
                    embedding[0] = 1.0;
                } else {
                    embedding[1] = 1.0;
                }

                let data: Vec<serde_json::Value> = inputs
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        serde_json::json!({
                            "object": "embedding",
                            "index": index,
                            "embedding": embedding,
                        })
                    })
                    .collect();

                let response = serde_json::to_vec(&serde_json::json!({
                    "object": "list",
                    "data": data,
                }))
                .expect("encode mock response");

                let headers = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    response.len()
                );

                stream.write_all(headers.as_bytes()).await.expect("write response headers");
                stream.write_all(&response).await.expect("write response body");
                stream.flush().await.expect("flush response");

                served += 1;
            }

            served
        });

        (endpoint, task)
    }

    #[tokio::test]
    async fn mock_server_accepts_grounded_report() {
        let report = "REPORT_MARKER The implementation uses the documented workspace interface.";
        let corpus =
            "CORPUS_MARKER The evidence documents the implementation and workspace interface.";

        let (endpoint, server) = mock_embed_server(true).await;

        let metrics = semantic_support_metrics_with(report, corpus, &endpoint, "mock-model")
            .await
            .expect("mock embeddings should produce metrics");

        assert!(
            (metrics.mean_support - 1.0).abs() < 1e-5,
            "expected full mean support, got {}",
            metrics.mean_support
        );
        assert!(
            (metrics.coverage - 1.0).abs() < 1e-5,
            "expected full coverage, got {}",
            metrics.coverage
        );

        let served = server.await.expect("mock server task");
        assert_eq!(served, 2, "expected corpus and report embedding requests");
    }

    #[tokio::test]
    async fn mock_server_rejects_ungrounded_report() {
        let report = "REPORT_MARKER The implementation uses an undocumented workspace interface.";
        let corpus =
            "CORPUS_MARKER The evidence documents an unrelated database migration process.";

        let (endpoint, server) = mock_embed_server(false).await;

        let metrics = semantic_support_metrics_with(report, corpus, &endpoint, "mock-model")
            .await
            .expect("mock embeddings should produce metrics");

        assert!(
            metrics.mean_support.abs() < 1e-5,
            "expected zero mean support, got {}",
            metrics.mean_support
        );
        assert!(metrics.coverage.abs() < 1e-5, "expected zero coverage, got {}", metrics.coverage);

        let served = server.await.expect("mock server task");
        assert_eq!(served, 2, "expected corpus and report embedding requests");
    }

    fn resolve_cal_path(path: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|x| x.parent())
                .expect("workspace root")
                .join(p)
        }
    }

    #[tokio::test]
    #[ignore = "manual calibration against explicitly supplied report and evidence files"]
    async fn calibrate_against_corpus() {
        let report_path = std::env::var("NEXUS_CAL_REPORT").expect("set NEXUS_CAL_REPORT");
        let corpus_path = std::env::var("NEXUS_CAL_CORPUS").expect("set NEXUS_CAL_CORPUS");
        let max_chunks: usize =
            std::env::var("NEXUS_CAL_MAX_CHUNKS").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
        let timeout_secs: u64 =
            std::env::var("NEXUS_CAL_TIMEOUT_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(90);

        let report = std::fs::read_to_string(&report_path).expect("read calibration report");
        let corpus = std::fs::read_to_string(&corpus_path).expect("read calibration evidence");

        // Cap the corpus input string so chunk() produces at most `max_chunks` chunks.
        let max_chars = max_chunks.saturating_mul(MAX_CHUNK_BYTES);
        let corpus: String = if corpus.chars().count() > max_chars {
            corpus.chars().take(max_chars).collect()
        } else {
            corpus
        };

        let endpoint = endpoint();
        let model = model();
        let batch = embed_batch_size();
        let report_chunks = chunk(&report).len();
        let corpus_chunks = chunk(&corpus).len();

        println!("=== CALIBRATION ===");
        println!("report: {report_path} ({} bytes, {report_chunks} chunks)", report.len());
        println!(
            "corpus: {corpus_path} ({} bytes, {corpus_chunks} chunks, cap {max_chunks})",
            corpus.len()
        );
        println!("endpoint: {endpoint}  model: {model}  batch: {batch}  timeout: {timeout_secs}s");
        println!("floors: mean>={MEAN_SUPPORT_FLOOR:.2}  coverage>={COVERAGE_FLOOR:.2}  per_chunk>={PER_CHUNK_SUPPORT_FLOOR:.2}");

        let metrics = semantic_support_metrics_with_timeout(
            &report,
            &corpus,
            &endpoint,
            &model,
            Duration::from_secs(timeout_secs),
        )
        .await;

        match metrics {
            Some(m) => {
                println!("mean_support:    {:.4}", m.mean_support);
                println!("coverage:        {:.4}", m.coverage);
                println!("weakest_support: {:.4}", m.weakest_support);
                println!("weakest_preview: {:?}", m.weakest_preview);
                let verdict =
                    if m.mean_support >= MEAN_SUPPORT_FLOOR && m.coverage >= COVERAGE_FLOOR {
                        "SUPPORTED (would pass)"
                    } else {
                        "BELOW THRESHOLD (would be rejected)"
                    };
                println!("verdict: {verdict}");
            }
            None => {
                println!("RESULT: FAIL-OPEN (endpoint down, timeout, malformed response, or count mismatch)");
            }
        }
    }
}
