use nexus_substrate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};

pub fn init() {
    tracing::info!(">> [DREAM_CYCLE] Subconscious synthesis engine initialised");
}

pub fn spawn(
    palace: Arc<RwLock<MemoryPalace>>,
    tx: broadcast::Sender<String>,
    client: reqwest::Client,
    api_key: String,
    orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30 * 60));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            tracing::info!("[DREAM_CYCLE] Beginning synthesis pass…");
            let _ = tx.send(
                serde_json::json!({"type":"LOG_EVENT","payload":"[DREAM] Synthesis cycle awakening…"})
                    .to_string()
            );

            let strands = {
                let mem = palace.read().await;
                mem.fetch_random_strands(8).await.unwrap_or_default()
            };

            if strands.is_empty() {
                continue;
            }

            let context = strands
                .iter()
                .map(|s| format!("DIRECTIVE: {}\nRESULT: {}", s.directive, s.result))
                .collect::<Vec<_>>()
                .join("\n---\n");

            let prompt = format!(
                "You are NEXUS Archivist. Analyse these memory strands and write one concise \
                 architectural insight (3-5 sentences, plain text, no JSON):\n\n{}",
                context
            );

            let ctx = nexus_model_hub::neural_agent::ContextPacket {
                system_prompt: Some("Output plain text only. One paragraph.".into()),
                model_override: Some("gemma-4-26b-a4b-it".into()),
                ..Default::default()
            };

            match orchestrator.run(prompt, ctx, &client, &api_key, Some(tx.clone())).await {
                Ok(insight) => {
                    let mem = palace.read().await;
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
