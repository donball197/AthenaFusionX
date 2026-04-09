// src/bin/summarizer_agent.rs - v1.2.3 [Borrow Checker Suture]
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};
use anyhow::{Result, Context};
use std::collections::HashMap;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Ignite environment variables
    dotenvy::dotenv().ok();

    println!("\x1b[1;35m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;35m║\x1b[0m \x1b[1;37mSUMMARIZER-AGENT: GEMMA 3 27B-it NEURAL LINK\x1b[0m                                \x1b[1;35m║\x1b[0m");
    println!("\x1b[1;35m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");

    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_multiplexed_async_connection().await?;
    let stream_key = "nexus_scrape_stream";

    println!(">> [SYNAPSE] Listening for new pulses on {}...", stream_key);

    loop {
        let opts = StreamReadOptions::default().count(1).block(5000);
        let reply: StreamReadReply = con.xread_options(&[stream_key], &["0"], &opts).await
            .context("!! [SYNAPSE_FRACTURE] Failed to read stream pulse.")?;

        for stream in reply.keys {
            for id in stream.ids {
                let map: HashMap<String, redis::Value> = id.map;
                
                let source: String = map.get("source")
                    .and_then(|v| redis::from_redis_value(v).ok())
                    .unwrap_or_else(|| "unknown".to_string());

                let payload: String = map.get("payload")
                    .and_then(|v| redis::from_redis_value(v).ok())
                    .unwrap_or_else(|| "empty_payload".to_string());

                let ts: String = map.get("timestamp")
                    .and_then(|v| redis::from_redis_value(v).ok())
                    .unwrap_or_else(|| "0".to_string());

                println!(">> [INGEST] Pulse ID: {}", id.id);
                println!(">> [INGEST] Source: \x1b[1;34m{}\x1b[0m", source);
                
                // Trigger the live API
                process_thinking_cycle(&payload, &ts).await?;
                
                println!("\x1b[32m>> [ZENITH] Summarization cycle complete.\x1b[0m");
            }
        }
        break; 
    }
    Ok(())
}

async fn process_thinking_cycle(payload: &str, timestamp: &str) -> Result<()> {
    let api_key = env::var("GEMMA_API_KEY").unwrap_or_default();
    
    if api_key.is_empty() {
        println!("!! [CORTEX_FRACTURE] GEMMA_API_KEY missing from environment.");
        return Ok(());
    }

    // Google AI Native Endpoint targeting Gemma 3 27B-it
    let api_url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);

    println!("\n>> [THINKING] <think>");
    println!(">> [UPLINK] Transmitting payload to Gemma 3 27B-it API...");

    let client = reqwest::Client::new();
    let prompt = format!(
        "Analyze this extracted web payload and provide a concise 2-sentence structural summary. Timestamp: {}\nPayload: {}",
        timestamp, payload
    );

    let request_body = json!({
        "contents": [{
            "parts": [{"text": prompt}]
        }]
    });

    let res = client.post(&api_url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if res.status().is_success() {
        let response_json: serde_json::Value = res.json().await?;
        let output = response_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("No response generated.");
        println!(">> [NEURAL_RESPONSE]\n{}", output);
    } else {
        // SUTURE: Save the status code before moving 'res' via .text()
        let status = res.status();
        let error_text = res.text().await.unwrap_or_default();
        println!("!! [API_FRACTURE] Google API returned status: {}\n{}", status, error_text);
    }

    println!(">> [THINKING] </think>");
    Ok(())
}
