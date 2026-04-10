/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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

        self.tx
            .send(payload.to_string())
            .map_err(|e| anyhow::anyhow!("Broadcast send failed: {}", e))?;

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}
