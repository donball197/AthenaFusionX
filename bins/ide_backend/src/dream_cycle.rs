use nexus_substrate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

pub async fn run(
    palace: Arc<MemoryPalace>,
    tx: broadcast::Sender<String>,
    client: reqwest::Client,
    api_key: String,
    orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let mut ticker = interval(Duration::from_secs(30 * 60));
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("[DREAM_CYCLE] Shutdown received");
                return Ok(());
            }
            _ = ticker.tick() => {}
        }

        let cycle = async {
            tracing::info!("[DREAM_CYCLE] Beginning synthesis pass…");
            let _ = tx.send(
                serde_json::json!({"type":"LOG_EVENT","payload":"[DREAM] Synthesis cycle awakening…"})
                    .to_string()
            );

            let strands = palace
                .fetch_random_strands_scoped(8, nexus_substrate::taint::FlowPolicy::INTERNAL_SINK)
                .await
                .unwrap_or_default();

            if strands.is_empty() {
                return;
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
                model_override: Some(
                    std::env::var("NEXUS_HEAVY_MODEL")
                        .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".into()),
                ),
                ..Default::default()
            };

            match orchestrator.run(prompt, ctx, &client, &api_key, Some(tx.clone())).await {
                Ok(insight) => {
                    if let Err(e) = palace.store_insight(&insight).await {
                        tracing::warn!("[DREAM_CYCLE] Failed to store insight: {}", e);
                    } else {
                        tracing::info!("[DREAM_CYCLE] Insight stored");
                    }
                }
                Err(e) => tracing::warn!("[DREAM_CYCLE] Synthesis failed: {}", e),
            }
        };

        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("[DREAM_CYCLE] Shutdown received during synthesis");
                return Ok(());
            }
            _ = cycle => {}
        }
    }
}
