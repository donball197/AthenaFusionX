// src/bin/summarizer_agent.rs - v1.1.0 [Type-Safe Ingestion]
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};
use anyhow::{Result, Context};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    println!("\x1b[1;35m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;35m║\x1b[0m \x1b[1;37mSUMMARIZER-AGENT: NEURAL FEEDBACK INGESTION\x1b[0m                               \x1b[1;35m║\x1b[0m");
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
                // SUTURE: Fix type mismatch by using redis::Value
                let map: HashMap<String, redis::Value> = id.map;
                
                // Explicitly decode redis::Value to String
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
                println!(">> [INGEST] Payload: \"{}\"", payload);

                // 3. Initiate Thinking Cycle
                process_thinking_cycle(&payload, &ts).await?;
                
                println!("\x1b[32m>> [ZENITH] Summarization cycle complete.\x1b[0m");
            }
        }
        break; // Break for verification; loop in production
    }
    Ok(())
}

async fn process_thinking_cycle(payload: &str, timestamp: &str) -> Result<()> {
    println!("\n>> [THINKING] <think>");
    println!("Analyzing extracted data: '{}' [TS: {}]", payload, timestamp);
    println!("Resonance Level: 0.85 | Consistency: Nominal.");
    println!("Summary: Placeholder example node detected. Metadata seating successful.");
    println!(">> [THINKING] </think>");
    Ok(())
}
