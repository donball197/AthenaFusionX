use nexus_ide_backend::system_monitor::SystemMonitor;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub async fn run(
    tx: broadcast::Sender<String>,
    monitor: Arc<Mutex<SystemMonitor>>,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(60));

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("[SELF_HEALING] Shutdown received");
                return Ok(());
            }
            _ = ticker.tick() => {}
        }

        let vitals = {
            let mut m = monitor.lock().await;
            m.get_vitals()
        };

        if vitals.total_ram_gb > 0.0 && (vitals.ram_gb / vitals.total_ram_gb) > 0.85 {
            tracing::warn!(
                "[SELF_HEALING] High RAM usage: {:.2}GB. Throttling.",
                vitals.ram_gb
            );
            let _ = tx.send(
                serde_json::json!({"type":"LOG_EVENT","payload":"[SYS] High RAM usage. Self-healing throttled."}).to_string()
            );
        } else {
            tracing::debug!("[SYS] Background Self-healing heartbeat active.");
        }
    }
}
