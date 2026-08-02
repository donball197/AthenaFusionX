//! Heartbeat — sends a periodic JSON ping over the broadcast channel so
//! the WebSocket watchdog in socket.ts never fires a stale-connection close.
use serde_json::json;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

pub fn init() {
    tracing::info!(">> [HEARTBEAT] Pulse initialised");
}

pub fn spawn(tx: broadcast::Sender<String>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let msg = json!({ "type": "HEARTBEAT", "ts": chrono_now() }).to_string();
            let _ = tx.send(msg);
        }
    });
}

fn chrono_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
