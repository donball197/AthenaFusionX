//! DreamCycle — background task that wakes every 30 minutes, pulls random
//! memory strands from the MemoryPalace, and asks the LLM to synthesise an
//! architectural insight which is stored back as a system_insight.
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};
use nexus_substrate::memory::MemoryPalace;

pub fn init() {
    tracing::info!(">> [DREAM_CYCLE] Subconscious synthesis engine initialised");
}

pub fn spawn(
    memory: Arc<Mutex<MemoryPalace>>,
    tx: broadcast::Sender<String>,
    client: reqwest::Client,
    api_key: String,
    orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30 * 60));
        ticker.tick().await; // skip immediate first tick

        loop {
            ticker.tick().await;
            tracing::info!("[DREAM_CYCLE] Beginning synthesis pass…");

            let _ = tx.send(
                serde_json::json!({"type":"LOG_EVENT","payload":"[DREAM] Synthesis cycle awakening…"})
                    .to_string()
            );

            let strands = {
                let mem = memory.lock().await;
                mem.fetch_random_strands(8).await.unwrap_or_default()
            };

            if strands.is_empty() {
                tracing::info!("[DREAM_CYCLE] No strands to synthesise");
                continue;
            }

            let context = strands.iter()
                .map(|s| format!("DIRECTIVE: {}\nRESULT: {}", s.directive, s.result))
                .collect::<Vec<_>>()
                .join("\n---\n");

            let prompt = format!(
                "You are NEXUS Archivist. Analyse these memory strands and write one concise \
                 architectural insight (3-5 sentences, plain text, no JSON):\n\n{}",
                context
            );

            // ✅ FIXED: ContextPacket comes from nexus_model_hub
            let ctx = nexus_model_hub::neural_agent::ContextPacket {
                system_prompt: Some("Output plain text only. One paragraph.".into()),
                model_override: Some("gemma-4-26b-a4b-it".into()),
                ..Default::default()
            };

            match orchestrator.run(prompt, ctx, &client, &api_key, Some(tx.clone())).await {
                Ok(insight) => {
                    let mem = memory.lock().await;
                    if let Err(e) = mem.store_insight(&insight).await {
                        tracing::warn!("[DREAM_CYCLE] Failed to store insight: {}", e);
                    } else {
                        tracing::info!("[DREAM_CYCLE] Insight stored");
                    }
                }
                Err(e) => tracing::warn!("[DREAM_CYCLE] Synthesis failed: {}", e),
            }
        }
    });
}
