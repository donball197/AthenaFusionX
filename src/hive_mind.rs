use redis::Client;
use anyhow::Result;
use serde_json::json;

pub struct HiveMind {
    pub client: Client,
}

impl HiveMind {
    pub fn connect() -> Result<Self> {
        let client = Client::open("redis://127.0.0.1/")?;
        Ok(Self { client })
    }

    pub async fn dispatch_task(&self, drone: &str, task: &str, params: &str) -> Result<()> {
        // PERF FIX: Asynchronous connection mapping
        let mut con = self.client.get_async_connection().await?;
        
        let payload = json!({
            "drone": drone,
            "task": task,
            "params": params,
            "timestamp": chrono::Utc::now().to_string()
        });
        
        // PERF FIX: Yields to Tokio runtime, saving ARM64 CPU cycles
        redis::cmd("PUBLISH")
            .arg("hive_bus")
            .arg(payload.to_string())
            .query_async::<_, ()>(&mut con)
            .await?;
            
        Ok(())
    }
}
