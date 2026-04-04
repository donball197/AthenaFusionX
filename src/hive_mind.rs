use anyhow::Result;
use serde_json::json;
use tokio::sync::broadcast;

/// HiveMind now uses the existing tokio broadcast bus instead of Redis.
/// Saves ~50MB RAM and removes the Redis process dependency entirely.
pub struct HiveMind {
    tx: broadcast::Sender<String>,
}

impl HiveMind {
    pub fn new(tx: broadcast::Sender<String>) -> Self {
        Self { tx }
    }

    pub async fn dispatch_task(&self, drone: &str, task: &str, params: &str) -> Result<()> {
        let payload = json!({
            "type": "HIVE_DISPATCH",
            "drone": drone,
            "task": task,
            "params": params,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        self.tx.send(payload.to_string())
            .map_err(|e| anyhow::anyhow!("Broadcast send failed: {}", e))?;

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}
