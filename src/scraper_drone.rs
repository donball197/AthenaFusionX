use crate::memory::MemoryPalace;
use anyhow::Result;
use scraper::{Html, Selector};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn execute_mission(url: &str, memory: Arc<Mutex<MemoryPalace>>) -> Result<String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".parse().unwrap());
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .default_headers(headers)
        .build()?;

    let response = client.get(url).send().await?.text().await?;

    let (title, full_strand) = {
        let document = Html::parse_document(&response);
        let title_selector = Selector::parse("title, h1").unwrap();
        let body_selector = Selector::parse("p, pre, code, article, .content").unwrap();

        let title_inner = document.select(&title_selector).next()
            .map(|e| e.text().collect::<String>()).unwrap_or_else(|| url.to_string());

        let mut content = Vec::new();
        for element in document.select(&body_selector).take(50) {
            let text: String = element.text().collect();
            let trimmed = text.trim();
            if trimmed.len() > 10 { content.push(trimmed.to_string()); }
        }
        (title_inner, content.join("\n\n"))
    };

    let result_summary = format!("TITLE: {}\nCONTENT_DENSITY: {} chars\nDATA: {}", title, full_strand.len(), full_strand);

    {
        let palace = memory.lock().await;
        palace.store_memory("ALPHA-SCOUT", &title, &result_summary, None).await?;
    }

    Ok(format!("SCRAPE_SUCCESS: Secured {} bytes", full_strand.len()))
}
