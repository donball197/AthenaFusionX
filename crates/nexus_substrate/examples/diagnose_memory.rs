use anyhow::{Context, Result};
use nexus_substrate::memory::MemoryPalace;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = "nexus_daemon.db";

    println!("==================================================");
    println!("PRODUCTION MEMORY VECTOR DIAGNOSTIC");
    println!("==================================================");
    println!("DB: {}", db_path);

    let palace = MemoryPalace::new(db_path)
        .await
        .context("failed to initialize MemoryPalace")?;

    let query =
        "machine uptime disk capacity available memory recent system observations";

    println!("QUERY: {}", query);

    let client = Client::new();

    let response = client
        .post("http://localhost:9380/v1/embeddings")
        .json(&serde_json::json!({
            "model": "granite-311m-edge",
            "input": query
        }))
        .send()
        .await
        .context("embedding gateway request failed")?
        .error_for_status()
        .context("embedding gateway returned HTTP error")?
        .json::<EmbeddingResponse>()
        .await
        .context("invalid embedding gateway response")?;

    let embedding = response
        .data
        .first()
        .context("embedding response contained no vectors")?
        .embedding
        .clone();

    println!("QUERY EMBEDDING DIMENSIONS: {}", embedding.len());

    if embedding.len() != 384 {
        anyhow::bail!(
            "expected 384-dimensional embedding, got {}",
            embedding.len()
        );
    }

    let memories = palace
        .semantic_search(&embedding, 3)
        .await
        .context("production semantic_search failed")?;

    println!();
    println!("==================================================");
    println!("PRODUCTION SEMANTIC SEARCH RESULTS");
    println!("==================================================");
    println!("RESULT COUNT: {}", memories.len());

    for (rank, memory) in memories.iter().enumerate() {
        let prefix: String = memory.result.chars().take(700).collect();

        println!();
        println!("---------------- RESULT {} ----------------", rank + 1);
        println!("SQLITE ID : {}", memory.id);
        println!("ORIGIN    : {}", memory.origin);
        println!("DIRECTIVE : {}", memory.directive);
        println!("RESULT LEN: {}", memory.result.len());
        println!("RESULT    : {}", prefix.replace('\n', "\\n"));
    }

    Ok(())
}
