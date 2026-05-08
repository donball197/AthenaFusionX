pub async fn fracture_backoff(attempt: u32) {
    let delay = 1000 * 3_u64.pow(attempt.saturating_sub(1));
    tracing::warn!("[BACKOFF] Waiting {}ms for Gemini to stabilize...", delay);
    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
}
