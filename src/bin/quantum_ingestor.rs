// src/bin/quantum_ingestor.rs — v1.2 [IMPORT_FIX + TYPE_FIX]
use anyhow::Result;
use nexus_substrate::memory::MemoryPalace;
use nexus_substrate::vector_bridge;
use nexus_singularity::neural_agent::{ContextPacket, MicroAgent};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

fn extract_last_json(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().rev() {
        if b == b'}' {
            end = Some(i);
            break;
        }
    }
    let end = end?;
    let mut depth: i32 = 0;
    let mut start = None;
    for (i, &b) in bytes[..=end].iter().enumerate().rev() {
        match b {
            b'}' => depth += 1,
            b'{' => {
                depth -= 1;
                if depth == 0 {
                    start = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let start = start?;
    let candidate = &text[start..=end];
    match serde_json::from_str::<serde_json::Value>(candidate) {
        Ok(v) if v.is_object() => {
            Some(serde_json::to_string(&v).unwrap_or_else(|_| candidate.to_string()))
        }
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("Failed to build HTTP client");
    let (tx, _) = broadcast::channel(16);
    let agent = Arc::new(MicroAgent::init(tx)?);

    // ⚡ FIX: Explicit type annotation
    let memory_palace: MemoryPalace = MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    println!("\x1b[35m>> [QUANTUM_INGESTOR] Initializing Physical PDF Extraction...\x1b[0m");

    let pdf_path = "Quantum_Machine_Learning_What_Quantum_Computing_Me.pdf";
    if !Path::new(pdf_path).exists() {
        eprintln!("\x1b[31m>> [FAULT] PDF not found at root.\x1b[0m");
        return Ok(());
    }

    let full_text = pdf_extract::extract_text(pdf_path)?;
    let chunk: String = full_text.chars().skip(4000).take(6000).collect();

    let prompt = format!(
        "Summarize this raw textbook extraction into a highly dense, technical JSON object \
         with exactly three keys: 'theory', 'classical_math', and 'quantum_bridge_potential'. \
         Strip out any OCR noise or page numbers. \
         Output ONLY the JSON object — no explanation, no markdown, no commentary.\n\nText:\n{}",
        chunk
    );

    let ctx = ContextPacket {
        system_prompt: Some("You are a Quantum Architect. JSON only.".to_string()),
        model_override: Some("gemma-4-26b-a4b-it".to_string()),
        ..Default::default()
    };

    println!("\x1b[36m>> [UPLINK] Dispatching chunk to gemma-4-26b-a4b-it...\x1b[0m");

    match agent.prepare_and_run(&client, &api_key, &prompt, ctx, 0.2, None).await {
        Ok((response, _)) => {
            let clean_json = match extract_last_json(&response) {
                Some(j) => j,
                None => response.trim().chars().take(2000).collect()
            };

            println!("\x1b[36m>> [UPLINK] Generating Semantic Vector Embeddings...\x1b[0m");

            // ⚡ FIX: Call vector_bridge from substrate and provide explicit type
            let embedding: Option<Vec<f32>> = vector_bridge::get_embedding(
                &api_key,
                &clean_json,
            )
            .await
            .ok();

            let palace = memory.lock().await;
            palace
                .store_memory(
                    "QUANTUM_INGESTOR",
                    "PDF_Extraction_Phase_1",
                    &clean_json,
                    embedding,
                )
                .await?;

            println!("\x1b[32m>> [SUCCESS] Clean JSON enshrined in Semantic Memory Palace.\x1b[0m");
        }
        Err(e) => eprintln!("\x1b[31m>> [FAULT] API Request Failed: {}\x1b[0m", e),
    }

    Ok(())
}
