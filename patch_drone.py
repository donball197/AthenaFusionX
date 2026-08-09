import os

print(">> Applying Scraper Drone Upgrade...")

path = "drones/data_scraper/src/main.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    old_func = """async fn scrape(State(_state): State<()>, Json(payload): Json<ScrapeRequest>) -> impl IntoResponse {
    Json(ScrapeResponse { content: format!("Nexus Drone acknowledged URL: {}", payload.url) })
}"""

    new_func = """async fn scrape(
    State(_state): State<()>,
    Json(payload): Json<ScrapeRequest>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let content = match client.get(&payload.url).send().await {
        Ok(resp) => resp.text().await.unwrap_or_else(|_| "Failed to decode body".to_string()),
        Err(e) => format!("Failed to fetch URL: {}", e),
    };
    Json(ScrapeResponse { content })
}"""

    if old_func in content:
        content = content.replace(old_func, new_func)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path} - Drone is now equipped with live web-scraping capabilities.")
    else:
        print(f"[!] Could not find target signature in {path}. It may already be patched.")
else:
    print(f"[ERROR] File not found: {path}")

