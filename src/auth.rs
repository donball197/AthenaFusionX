use anyhow::Result;

/// verify_sovereign_link: Validates a bearer token against the local
/// NEXUS_AUTH_KEY environment variable. Designed for LAN-only use.
pub async fn verify_sovereign_link(token: &str) -> Result<bool> {
    let key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_else(|_| "SOVEREIGN".into());
    Ok(token == key)
}
