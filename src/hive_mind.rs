use redis::Client;
use anyhow::Result;
use serde_json::json;

pub struct HiveMind {
    pub client: Client, // Added pub so it is read
}

impl HiveMind {
    pub fn connect() -> Result<Self> {
        let client = Client::open("redis://127.0.0.1/")?;
        Ok(Self { client })
    }

    pub async fn dispatch_task(&self, drone: &str, task: &str, params: &str) -> Result<()> {
        let mut con = self.client.get_connection()?;
        let payload = json!({
            "drone": drone,
            "task": task,
            "params": params,
            "timestamp": chrono::Utc::now().to_string()
        });
        
        // FIX: Added ::<()> to specify the return type explicitly
        redis::cmd("PUBLISH")
            .arg("hive_bus")
            .arg(payload.to_string())
            .query::<()>(&mut con)?;
            
        Ok(())
    }
}
