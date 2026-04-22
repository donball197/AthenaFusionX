// src/scraper_drone.rs — v2.1
// FIX: All .parse().unwrap() chains replaced with map_err / ?.
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use scraper::{Html, Selector};
use crate::memory::MemoryPalace;

pub async fn execute_mission(url: &str, memory: Arc<Mutex<MemoryPalace>>) -> Result<String> {
    // Build a browser-like client to avoid basic WAF blocks
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            .parse()
            .map_err(|e| anyhow::anyhow!("Header parse error: {}", e))?,
    );
    headers.insert(
        "Accept-Language",
        "en-US,en;q=0.5"
            .parse()
            .map_err(|e| anyhow::anyhow!("Header parse error: {}", e))?,
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36")
        .default_headers(headers)
        .build()?;

    let response = client.get(url).send().await?.text().await?;

    let (title, full_strand) = {
        let document = Html::parse_document(&response);

        // FIX: Selector::parse unwrap → expect with descriptive message
        let title_selector = Selector::parse("title, h1")
            .expect("[SCRAPER] 'title, h1' is a valid CSS selector — compile-time bug");
        let body_selector = Selector::parse("p, pre, code, article, .content")
            .expect("[SCRAPER] body selector is valid — compile-time bug");

        let title_inner = document
            .select(&title_selector)
            .next()
            .map(|e| e.text().collect::<String>())
            .unwrap_or_else(|| url.to_string());

        let mut content = Vec::new();
        for element in document.select(&body_selector).take(50) {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if trimmed.len() > 10 {
                content.push(trimmed.to_string());
            }
        }
        (title_inner, content.join("\n\n"))
    };

    if full_strand.is_empty() {
        return Err(anyhow::anyhow!(
            "SCRAPE_FAULT: Page returned no readable text. May be JS-rendered or blocked."
        ));
    }

    let result_summary = format!(
        "TITLE: {}\nCONTENT_DENSITY: {} chars\nDATA: {}",
        title,
        full_strand.len(),
        full_strand
    );

    {
        let palace = memory.lock().await;
        palace.store_memory("ALPHA-SCOUT", &title, &result_summary).await?;
    }

    Ok(format!(
        "SCRAPE_SUCCESS: Secured {} bytes from {} and stored in MemoryPalace",
        full_strand.len(),
        url
    ))
}
