// src/bin/summarizer_agent.rs - v1.5.0 [Daemon State & Consumer Groups]
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};
use anyhow::{Result, Context};
use std::collections::HashMap;
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use nexus_singularity::memory::MemoryPalace;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    println!("\x1b[1;35m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;35m║\x1b[0m \x1b[1;37mSUMMARIZER-AGENT: DAEMON STATE [CONSUMER GROUP ACTIVE]\x1b[0m                      \x1b[1;35m║\x1b[0m");
    println!("\x1b[1;35m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");

    // 1. Ignite MemoryPalace
    let db_path = env::var("DATABASE_URL").unwrap_or_else(|_| ".nexus_memory.db".to_string());
    let clean_path = db_path.replace("sqlite:", "");
    
    println!(">> [CORTEX] Initializing MemoryPalace at {}...", clean_path);
    let memory = MemoryPalace::new(&clean_path).await
        .context("!! [CORTEX_FRACTURE] SQLite MemoryPalace ignition failed.")?;
    let memory_arc = Arc::new(Mutex::new(memory));

    // 2. Connect to the Synapse
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_multiplexed_async_connection().await?;
    let stream_key = "nexus_scrape_stream";
    let group_name = "summarizer_cortex";
    let consumer_name = "duet_node_alpha";

    // 3. Register Consumer Group (Ignores error if group already exists)
    let group_res: redis::RedisResult<()> = con.xgroup_create_mkstream(stream_key, group_name, "0").await;
    if let Err(e) = group_res {
        if !e.to_string().contains("BUSYGROUP") {
            println!("!! [SYNAPSE_WARN] Group creation anomaly: {}", e);
        }
    }

    println!(">> [SYNAPSE] Daemon active. Listening for new pulses as '{}'...", consumer_name);

    // 4. Continuous Background Ingestion Loop
    loop {
        // Block for 5 seconds waiting for new data, then loop. 
        // Using ">" means "read messages never delivered to this consumer group"
        let opts = StreamReadOptions::default().group(group_name, consumer_name).count(1).block(5000);
        let reply: redis::RedisResult<StreamReadReply> = con.xread_options(&[stream_key], &[">"], &opts).await;

        match reply {
            Ok(stream_reply) => {
                for stream in stream_reply.keys {
                    for id in stream.ids {
                        let map: HashMap<String, redis::Value> = id.map;
                        
                        let source: String = map.get("source").and_then(|v| redis::from_redis_value(v).ok()).unwrap_or_default();
                        let payload: String = map.get("payload").and_then(|v| redis::from_redis_value(v).ok()).unwrap_or_default();
                        let ts: String = map.get("timestamp").and_then(|v| redis::from_redis_value(v).ok()).unwrap_or_default();

                        if payload.is_empty() { continue; } // Skip empty pulses

                        println!(">> [INGEST] Pulse ID: {}", id.id);
                        println!(">> [INGEST] Source: \x1b[1;34m{}\x1b[0m", source);
                        
                        // 5. Trigger the live API
                        if let Err(e) = process_thinking_cycle(&payload, &ts, &source, memory_arc.clone()).await {
                            println!("!! [CORTEX_FRACTURE] Thinking cycle failed: {}", e);
                            continue; // Skip XACK so it can be retried later
                        }
                        
                        // 6. The Acknowledgement Suture (Marks pulse as digested)
                        let _ : redis::RedisResult<()> = con.xack(stream_key, group_name, &[&id.id]).await;
                        println!("\x1b[32m>> [ZENITH] Cycle complete. Pulse {} acknowledged and purged from pending list.\x1b[0m\n", id.id);
                    }
                }
            },
            Err(e) => {
                // Ignore standard timeout errors from blocking read
                if !e.is_timeout() {
                    println!("!! [SYNAPSE_FRACTURE] Stream read error: {}", e);
                }
            }
        }
    }
}

async fn process_thinking_cycle(payload: &str, timestamp: &str, source: &str, memory_arc: Arc<Mutex<MemoryPalace>>) -> Result<()> {
    let api_key = env::var("GEMMA_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        println!("!! [CORTEX_FRACTURE] GEMMA_API_KEY missing from environment.");
        return Ok(());
    }

    let model = env::var("NEXUS_MODEL").unwrap_or_else(|_| "gemma-4-26b-a4b-it".to_string());
    let api_url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);

    println!(">> [THINKING] <think>");
    println!(">> [UPLINK] Transmitting payload to {} API...", model);

    let client = reqwest::Client::new();
    let prompt = format!(
        "Analyze this extracted web payload and provide a concise 2-sentence structural summary. Timestamp: {}\nPayload: {}",
        timestamp, payload
    );

    let request_body = json!({ "contents": [{ "parts": [{"text": prompt}] }] });
    let res = client.post(&api_url).header("Content-Type", "application/json").json(&request_body).send().await?;

    if res.status().is_success() {
        let response_json: serde_json::Value = res.json().await?;
        let output = response_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str().unwrap_or("No response generated.").to_string(); 
            
        println!(">> [NEURAL_RESPONSE]\n{}", output);

        let memory_lock = memory_arc.lock().await;
        let directive = "Web Payload Summarization";
        memory_lock.store_memory(source, directive, &output).await?;
        println!(">> [IMPRINT] Knowledge seeded into neural_context index.");

    } else {
        let status = res.status();
        let error_text = res.text().await.unwrap_or_default();
        anyhow::bail!("API returned status {}: {}", status, error_text);
    }

    println!(">> [THINKING] </think>");
    Ok(())
}
