//! Heartbeat — sends a periodic JSON ping over the broadcast channel so
//! the WebSocket watchdog in socket.ts never fires a stale-connection close.
use serde_json::json;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

pub async fn run(
    tx: broadcast::Sender<String>,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let mut ticker = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("[HEARTBEAT] Shutdown received");
                return Ok(());
            }
            _ = ticker.tick() => {}
        }

        let msg = json!({ "type": "HEARTBEAT", "ts": chrono_now() }).to_string();
        let _ = tx.send(msg);
    }
}

fn chrono_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
