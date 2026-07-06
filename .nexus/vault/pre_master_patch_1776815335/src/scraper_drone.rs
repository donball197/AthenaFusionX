use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use scraper::{Html, Selector};
use crate::memory::MemoryPalace;

pub async fn execute_mission(url: &str, memory: Arc<Mutex<MemoryPalace>>) -> Result<String> {
    // ⚡ FIX: Spoof a standard Windows Chrome browser to bypass basic WAF/Cloudflare blocks
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".parse().unwrap());
            headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
            headers
        })
        .build()?;

    let response = client.get(url).send().await?.text().await?;

    let (title, full_strand) = {
        let document = Html::parse_document(&response);

        let title_selector = Selector::parse("title, h1").unwrap();
        let title_inner = document.select(&title_selector).next()
            .map(|e| e.text().collect::<String>())
            .unwrap_or_else(|| url.to_string());

        // Expanded selector to catch more article bodies
        let body_selector = Selector::parse("p, pre, code, article, .content").unwrap();
        let mut content = Vec::new();

        for element in document.select(&body_selector).take(50) {
            let text: String = element.text().collect::<String>();
            let trimmed = text.trim();
            // Lowered density threshold to catch smaller paragraphs
            if trimmed.len() > 10 {
                content.push(trimmed.to_string());
            }
        }
        (title_inner, content.join("\n\n"))
    };

    if full_strand.is_empty() {
        return Err(anyhow::anyhow!("SCRAPE_FAULT: Page returned no readable text. It may be heavily JavaScript-rendered or completely blocked."));
    }

    let result_summary = format!("TITLE: {}\nCONTENT_DENSITY: {} chars\nDATA: {}", title, full_strand.len(), full_strand);

    let palace = memory.lock().await;
    palace.store_memory("ALPHA-SCOUT", &title, &result_summary).await?;

    Ok(format!("SCRAPE_SUCCESS: Secured {} bytes from {} and stored in MemoryPalace", full_strand.len(), url))
}
