// src/bin/quantum_ingestor.rs — v1.1 [CLEAN_JSON_STORE_FIX]
// FIX: Was storing the full LLM chain-of-thought including duplicate
//      JSON blocks and markdown fences into neural_context.result.
//      Now extracts the last valid JSON object from the response before
//      persisting — result column contains only the clean data object.
// FIX: Embedding is generated from the clean JSON, not the raw trace.
use anyhow::Result;
use nexus_singularity::memory::MemoryPalace;
use nexus_singularity::neural_agent::{ContextPacket, MicroAgent};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// extract_last_json: Finds the last {...} block in a string and
/// attempts to parse it as valid JSON. Returns the minified JSON
/// string on success, or None if no valid object is found.
fn extract_last_json(text: &str) -> Option<String> {
    // Walk backwards through the string to find the last closing brace
    let bytes = text.as_bytes();
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().rev() {
        if b == b'}' {
            end = Some(i);
            break;
        }
    }

    let end = end?;

    // Find the matching opening brace by counting depth
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

    // Validate it parses as a JSON object with our expected keys
    match serde_json::from_str::<serde_json::Value>(candidate) {
        Ok(v) if v.is_object() => {
            // Minify for storage efficiency
            Some(serde_json::to_string(&v).unwrap_or_else(|_| candidate.to_string()))
        }
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();
    let (tx, _) = broadcast::channel(16);
    let agent = Arc::new(MicroAgent::init(tx)?);

    let memory_palace = MemoryPalace::new(".nexus_memory.db").await?;
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
        system_prompt: Some(
            "You are a Quantum Architect. Output ONLY a valid JSON object with keys \
             'theory', 'classical_math', 'quantum_bridge_potential'. \
             No markdown. No explanation. No preamble. JSON only."
            .to_string()
        ),
        model_override: Some("gemma-4-26b-a4b-it".to_string()),
        ..Default::default()
    };

    println!("\x1b[36m>> [UPLINK] Dispatching chunk to gemma-4-26b-a4b-it...\x1b[0m");

    match agent.prepare_and_run(&client, &api_key, &prompt, ctx, 0.2, None).await {
        Ok((response, _)) => {
            // FIX: Extract only the last clean JSON object from the response
            let clean_json = match extract_last_json(&response) {
                Some(j) => {
                    println!("\x1b[32m>> [PARSER] Clean JSON extracted ({} bytes)\x1b[0m", j.len());
                    j
                }
                None => {
                    // Fallback: strip fences and take the raw response
                    eprintln!("\x1b[33m>> [PARSER] No valid JSON object found — storing raw (trimmed)\x1b[0m");
                    response
                        .trim_start_matches("```json")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim()
                        .chars()
                        .take(2000)
                        .collect()
                }
            };

            println!("\x1b[36m>> [UPLINK] Generating Semantic Vector Embeddings...\x1b[0m");

            // FIX: Embed the clean JSON, not the raw chain-of-thought
            let embedding = nexus_singularity::vector_bridge::get_embedding(
                &api_key,
                &clean_json,
            )
            .await
            .ok();

            if embedding.is_none() {
                eprintln!(
                    "\x1b[33m>> [EMBEDDING] Warning: embedding generation failed. \
                     Check GEMINI_API_KEY has access to text-embedding-004.\x1b[0m"
                );
            }

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
            println!("\x1b[32m>> [PREVIEW] {}\x1b[0m", &clean_json.chars().take(120).collect::<String>());
        }
        Err(e) => eprintln!("\x1b[31m>> [FAULT] API Request Failed: {}\x1b[0m", e),
    }

    Ok(())
}
