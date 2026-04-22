use anyhow::{anyhow, Result};

/// verify_sovereign_link: Validates a bearer token against NEXUS_AUTH_KEY.
/// Fails loudly if the env var is not set — no insecure hardcoded fallback.
pub async fn verify_sovereign_link(token: &str) -> Result<bool> {
    let key = std::env::var("NEXUS_AUTH_KEY")
        .map_err(|_| anyhow!("NEXUS_AUTH_KEY is not set. Cannot verify sovereign link."))?;

    // Constant-time comparison to prevent timing attacks
    Ok(constant_time_eq(token.as_bytes(), key.as_bytes()))
}

/// Constant-time byte comparison (avoids early-exit timing leaks)
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
