/**
 * Copyright 2026 Donald Ball
 * Licensed under the Apache License, Version 2.0
 */

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use scraper::{Html, Selector};
use crate::memory::MemoryPalace;

pub async fn execute_mission(url: &str, memory: Arc<Mutex<MemoryPalace>>) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("NEXUS-AlphaScout/4.2 (Sovereign Architecture)")
        .build()?;

    let response = client.get(url).send().await?.text().await?;

    // --- SUTURE: Non-Atomic Scope Block (Thread Safety Fix) ---
    let (title, full_strand) = {
        let document = Html::parse_document(&response);
        
        let title_selector = Selector::parse("h1").unwrap();
        let title_inner = document.select(&title_selector).next()
            .map(|e| e.text().collect::<String>())
            .unwrap_or_else(|| url.to_string());

        let body_selector = Selector::parse("p, pre, code").unwrap();
        let mut content = Vec::new();
        
        for element in document.select(&body_selector).take(50) {
            let text: String = element.text().collect::<String>();
            let trimmed = text.trim();
            if trimmed.len() > 20 {
                content.push(trimmed.to_string());
            }
        }
        (title_inner, content.join("\n\n"))
    }; 

    let result_summary = format!("TITLE: {}\nCONTENT_DENSITY: {} chars\nDATA: {}", title, full_strand.len(), full_strand);

    // Imprint to Synapse
    let palace = memory.lock().await;
    palace.store_memory("ALPHA-SCOUT", &title, &result_summary).await?;

    Ok(format!("SCRAPE_SUCCESS: Secured {} bytes from {}", full_strand.len(), url))
}
