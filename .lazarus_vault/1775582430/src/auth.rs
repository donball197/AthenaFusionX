use crate::nexus_error::NexusError;

/// verify_sovereign_link: Validates a bearer token against the local
/// NEXUS_AUTH_KEY environment variable. Designed for LAN-only use.
///
/// FIX: Removed the dead `use ax_auth::NexusIdentity` import — that crate
/// does not exist in the dependency tree and would cause a compile error.
pub async fn verify_sovereign_link(token: &str) -> Result<bool, NexusError> {
    let key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_else(|_| "SOVEREIGN".into());
    Ok(token == key)
}
