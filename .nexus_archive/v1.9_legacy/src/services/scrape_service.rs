use anyhow::{anyhow, Result};
use tokio::process::Command;

pub async fn scrape_url(url: &str) -> Result<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("PATH=/usr/bin:/bin curl -fsSL --max-time 10 '{}' | sed 's/<[^>]*>//g' | head -c 5000", url))
        .output().await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!("CURL_FAIL: {}", String::from_utf8_lossy(&output.stderr)))
    }
}
