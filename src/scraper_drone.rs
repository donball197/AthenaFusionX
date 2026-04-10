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


// src/scraper_drone.rs - ALPHA-SCOUT (Natively Tethered)
use crate::memory::MemoryPalace;
use crate::resonator::ResonatorCore;
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn execute_mission(url: &str, memory: Arc<Mutex<MemoryPalace>>) -> Result<()> {
    println!(">> [ALPHA-SCOUT] Traversal: {}", url);

    let client = reqwest::Client::builder().user_agent("NexusSingularity/4.0.0").build()?;
    let response = client.get(url).send().await.context("Network uplink failed")?;
    let html_bytes = response.bytes().await?;

    println!(">> [ALPHA-SCOUT] Extracting <h1> via Zig Resonator...");
    let extracted = ResonatorCore::execute_scrape(&html_bytes, "<h1>");

    if let Some(content) = extracted {
        println!(">> [ALPHA-SCOUT] Pulse captured. Committing to SQLite Synapse...");
        let palace = memory.lock().await;
        // SUTURE: Updated to use the universal v4 store_memory
        palace.store_memory("ALPHA_SCOUT", url, &content).await?;
    }

    Ok(())
}
