//! SelfHealingEngine — polls `cargo check` on a slow interval and, if errors
//! are found, fires a RepoDoctor directive through the agent pipeline.
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};
use nexus_substrate::memory::MemoryPalace;
use nexus_runtime_core::shared_state::VmoState;
use nexus_runtime_core::distributed_scheduler::DistributedScheduler;

pub fn init() {
    tracing::info!(">> [SELF_HEALING] Suture engine initialised");
}

pub fn spawn(
    orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
    client: reqwest::Client,
    api_key: String,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) {
    tokio::spawn(async move {
        // First run after 5 minutes, then every 20 minutes
        tokio::time::sleep(Duration::from_secs(5 * 60)).await;
        let mut ticker = interval(Duration::from_secs(20 * 60));
        loop {
            ticker.tick().await;
            tracing::info!("[SELF_HEALING] Running cargo check…");

            let output = tokio::process::Command::new("cargo")
                .args(["check", "--message-format=short", "--quiet"])
                .output()
                .await;

            match output {
                Ok(out) if out.status.success() => {
                    tracing::info!("[SELF_HEALING] Substrate integrity verified ✓");
                    let _ = tx.send(
                        serde_json::json!({"type":"LOG_EVENT","payload":"[HEAL] cargo check: PASS"})
                            .to_string()
                    );
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    tracing::warn!("[SELF_HEALING] Errors detected — triggering RepoDoctor");
                    let _ = tx.send(
                        serde_json::json!({"type":"LOG_EVENT",
                            "payload": format!("[HEAL] cargo check FAILED — engaging RepoDoctor")})
                            .to_string()
                    );
                    let directive = format!(
                        "diagnose and fix the following cargo check errors:\n{}",
                        &stderr[..stderr.len().min(2000)]
                    );
                    nexus_runtime_core::task_manager::send_task(
                        orchestrator.clone(), &client, &api_key, &directive,
                        memory.clone(), vmo_state.clone(), tx.clone(), dist.clone()
                    ).await;
                }
                Err(e) => tracing::warn!("[SELF_HEALING] Could not run cargo check: {}", e),
            }
        }
    });
}
