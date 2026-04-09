// src/synapse.rs - v3.1 DEFINITIVE REDIS UPLINK
use redis::AsyncCommands;
use anyhow::{Result, Context};

pub struct NexusSynapse {
    client: redis::Client,
    pub stream_key: String,
}

impl NexusSynapse {
    pub fn ignite(redis_url: &str, stream_key: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .context("Failed to connect to nexus_synapse container")?;
            
        Ok(Self {
            client,
            stream_key: stream_key.to_string(),
        })
    }

    pub async fn stream_payload(&self, source_url: &str, payload: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .context("Failed to establish async multiplexed connection")?;
        
        let timestamp = chrono::Utc::now().to_rfc3339();
        let fields = [
            ("source", source_url), 
            ("payload", payload), 
            ("timestamp", &timestamp)
        ];

        let _: String = conn.xadd(&self.stream_key, "*", &fields).await
            .context("Failed to stream payload to Redis")?;

        println!(">> [SYNAPSE] Payload from {} streamed to {}.", source_url, self.stream_key);
        Ok(())
    }
}
