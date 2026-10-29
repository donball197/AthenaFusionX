use nexus_ide_backend::system_monitor::SystemMonitor;
use nexus_runtime_core::distributed_scheduler::DistributedScheduler;
use nexus_runtime_core::memory_cortex::MemoryCortex;
use nexus_runtime_core::shared_state::VmoState;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub fn init() {
    tracing::info!(">> [SELF_HEALING] Suture engine initialised");
}

#[allow(clippy::too_many_arguments)]
pub fn spawn(
    _orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
    _client: reqwest::Client,
    _api_key: String,
    _memory: Arc<MemoryCortex>,
    _vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    _dist: Arc<DistributedScheduler>,
    monitor: Arc<Mutex<SystemMonitor>>,
) {
    tokio::spawn(async move {
        // Ticks once a minute
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            ticker.tick().await;
            let vitals = {
                let mut m = monitor.lock().await;
                m.get_vitals()
            };

            // If RAM hits 85%, warn the user in the CLI UI
            if vitals.total_ram_gb > 0.0 && (vitals.ram_gb / vitals.total_ram_gb) > 0.85 {
                tracing::warn!(
                    "[SELF_HEALING] High RAM usage: {:.2}GB. Throttling.",
                    vitals.ram_gb
                );
                let _ = tx.send(
                    serde_json::json!({"type":"LOG_EVENT","payload":"[SYS] High RAM usage. Self-healing throttled."}).to_string()
                );
            } else {
                // Otherwise, log quietly to daemon.log and do not pollute the CLI
                tracing::debug!("[SYS] Background Self-healing heartbeat active.");
            }
        }
    });
}
